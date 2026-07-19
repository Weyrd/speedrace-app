use super::ffmpeg::{resolve_ffmpeg_path, spawn_ffmpeg};
use super::Encoder;
use crate::logging::{mlog, LogCat};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

const PROBE_W: u32 = 640;
const PROBE_H: u32 = 360;
const PROBE_FRAMES: usize = 4;
const PROBE_TIMEOUT: Duration = Duration::from_secs(6);

// `ffmpeg -encoders` is useless here: it lists what was COMPILED in, not what this machine can
// actually open. The sidecar ships nvenc+amf unconditionally, so only a real encode answers.
static CAPS: OnceLock<Mutex<HashMap<(Encoder, u8), bool>>> = OnceLock::new();

// Only completed probes are cached, so overlapping warms would trial-encode at the same time
// and starve each other of GPU sessions, caching a working encoder as unusable.
static PROBE_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

fn caps() -> &'static Mutex<HashMap<(Encoder, u8), bool>> {
    CAPS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn probe_lock() -> &'static tokio::sync::Mutex<()> {
    PROBE_LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

fn cached(enc: Encoder, legs: u8) -> Option<bool> {
    caps().lock().ok()?.get(&(enc, legs)).copied()
}

fn remember(enc: Encoder, legs: u8, ok: bool) {
    if let Ok(mut m) = caps().lock() {
        m.insert((enc, legs), ok);
    }
}

// A live failure invalidates the cache: the GPU had a session when we probed and lost it since.
pub fn poison(enc: Encoder) {
    if let Ok(mut m) = caps().lock() {
        for legs in 1..=2u8 {
            m.insert((enc, legs), false);
        }
    }
    mlog!(LogCat::Stream, "[encoder] {} poisoned", enc.name());
}

// Mirrors the real pipeline shape: a GPU with no free session passes a 1-leg probe and then
// fails live, because we open two encoder instances (WHIP + replay) in one process.
fn probe_args(enc: Encoder, legs: u8) -> Vec<String> {
    let mut a: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostats",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "bgra",
        "-video_size",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    a.push(format!("{PROBE_W}x{PROBE_H}"));
    for s in ["-framerate", "30", "-i", "pipe:0"] {
        a.push(s.to_string());
    }
    if legs == 2 {
        a.push("-filter_complex".into());
        a.push("[0:v]format=yuv420p,split=2[a][b]".into());
    } else {
        a.push("-vf".into());
        a.push("format=yuv420p".into());
    }
    for i in 0..legs {
        if legs == 2 {
            a.push("-map".into());
            a.push(if i == 0 { "[a]".into() } else { "[b]".into() });
        }
        a.push("-c:v".into());
        a.push(enc.name().to_string());
        a.push("-b:v".into());
        a.push("1000k".into());
        a.push("-f".into());
        a.push("mp4".into());
        a.push("-movflags".into());
        a.push("+frag_keyframe+empty_moov".into());
        a.push(if i == 0 {
            "pipe:1".into()
        } else {
            super::NULL_SINK.to_string()
        });
    }
    a
}

async fn probe(enc: Encoder, legs: u8) -> bool {
    if let Some(v) = cached(enc, legs) {
        return v;
    }
    let _serialized = probe_lock().lock().await;
    if let Some(v) = cached(enc, legs) {
        return v;
    }
    let Ok(path) = resolve_ffmpeg_path() else {
        return false;
    };
    let args = probe_args(enc, legs);
    let Ok(mut child) = spawn_ffmpeg(&path, &args) else {
        remember(enc, legs, false);
        return false;
    };

    if let Some(mut stdin) = child.stdin.take() {
        let frame = vec![0u8; (PROBE_W * PROBE_H * 4) as usize];
        tauri::async_runtime::spawn(async move {
            for _ in 0..PROBE_FRAMES {
                if stdin.write_all(&frame).await.is_err() {
                    return;
                }
            }
            let _ = stdin.shutdown().await;
        });
    }

    let ok = match tokio::time::timeout(PROBE_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status.success(),
        _ => {
            let _ = child.kill().await;
            false
        }
    };
    mlog!(
        LogCat::Stream,
        "[encoder] probe {} legs={legs} -> {}",
        enc.name(),
        if ok { "usable" } else { "unusable" }
    );
    remember(enc, legs, ok);
    // Two sessions opening means one will too, and one failing means two cannot. Lets a
    // warm at either leg count answer the other, instead of probing inside the 25s publish.
    match (legs, ok) {
        (2, true) => remember(enc, 1, true),
        (1, false) => remember(enc, 2, false),
        _ => {}
    }
    ok
}

// Runs the probe ahead of time so publish never pays for it.
pub async fn warm(with_replay: bool) {
    let legs = if with_replay { 2 } else { 1 };
    if probe(Encoder::Nvenc, legs).await {
        return;
    }
    probe(Encoder::Amf, legs).await;
}

pub async fn select(pref: Option<Encoder>, with_replay: bool) -> Encoder {
    let legs = if with_replay { 2 } else { 1 };
    match pref {
        Some(Encoder::X264) => Encoder::X264,
        // An explicit pick is still verified: a forced-but-broken encoder would fail at publish.
        Some(enc) => pick(enc, legs).await,
        None => {
            if probe(Encoder::Nvenc, legs).await {
                Encoder::Nvenc
            } else if probe(Encoder::Amf, legs).await {
                Encoder::Amf
            } else {
                Encoder::X264
            }
        }
    }
}

async fn pick(enc: Encoder, legs: u8) -> Encoder {
    if probe(enc, legs).await {
        enc
    } else {
        mlog!(
            LogCat::Stream,
            "[encoder] {} forced but unusable; using libx264",
            enc.name()
        );
        Encoder::X264
    }
}

// What Auto would choose right now, for the settings UI. None while the probe is still cold.
pub fn detected() -> Option<Encoder> {
    let m = caps().lock().ok()?;
    for enc in [Encoder::Nvenc, Encoder::Amf] {
        match m.get(&(enc, 2)).or_else(|| m.get(&(enc, 1))) {
            Some(true) => return Some(enc),
            Some(false) => continue,
            None => return None,
        }
    }
    Some(Encoder::X264)
}
