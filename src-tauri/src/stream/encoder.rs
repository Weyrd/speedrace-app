use super::ffmpeg::{resolve_ffmpeg_path, spawn_ffmpeg};
use super::{Encoder, Rung, StreamSettings};
use crate::logging::{mlog, LogCat};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PROBE_W: u32 = 640;
const PROBE_H: u32 = 360;
const PROBE_FRAMES: u32 = 4;
const PROBE_TIMEOUT: Duration = Duration::from_secs(6);
const MIN_PROBE_BYTES: u64 = 512;

static CAPS: OnceLock<Mutex<HashMap<(Encoder, u8), bool>>> = OnceLock::new();
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

pub fn poison(enc: Encoder) {
    if let Ok(mut m) = caps().lock() {
        for legs in 1..=2u8 {
            m.insert((enc, legs), false);
        }
    }
    mlog!(LogCat::Stream, "[encoder] {} poisoned", enc.name());
}

fn synthetic_frame(width: u32, height: u32, seed: u32) -> Vec<u8> {
    let mut buf = vec![0u8; (width * height * 4) as usize];
    for (i, px) in buf.chunks_exact_mut(4).enumerate() {
        let x = i as u32 % width;
        let y = i as u32 / width;
        let v = (x ^ y).wrapping_add(seed).wrapping_mul(37) as u8;
        px[0] = v;
        px[1] = v.wrapping_add(85);
        px[2] = v.wrapping_add(170);
        px[3] = 255;
    }
    buf
}

async fn drain_stdout(stdout: tokio::process::ChildStdout) -> u64 {
    let mut reader = stdout;
    let mut buf = [0u8; 8192];
    let mut total = 0u64;
    loop {
        match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n as u64,
        }
    }
    total
}

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

    let stdout_task = child
        .stdout
        .take()
        .map(|out| tauri::async_runtime::spawn(drain_stdout(out)));

    if let Some(mut stdin) = child.stdin.take() {
        tauri::async_runtime::spawn(async move {
            for i in 0..PROBE_FRAMES {
                let frame = synthetic_frame(PROBE_W, PROBE_H, i);
                if stdin.write_all(&frame).await.is_err() {
                    return;
                }
            }
            let _ = stdin.shutdown().await;
        });
    }

    let exited = match tokio::time::timeout(PROBE_TIMEOUT, child.wait()).await {
        Ok(Ok(status)) => status.success(),
        _ => {
            let _ = child.kill().await;
            false
        }
    };
    let encoded_bytes = match stdout_task {
        Some(task) => tokio::time::timeout(Duration::from_millis(500), task)
            .await
            .ok()
            .and_then(|r| r.ok())
            .unwrap_or(0),
        None => 0,
    };
    let ok = exited && encoded_bytes >= MIN_PROBE_BYTES;
    mlog!(
        LogCat::Stream,
        "[encoder] probe {} legs={legs} -> {} ({encoded_bytes}B)",
        enc.name(),
        if ok { "usable" } else { "unusable" }
    );
    remember(enc, legs, ok);
    match (legs, ok) {
        (2, true) => remember(enc, 1, true),
        (1, false) => remember(enc, 2, false),
        _ => {}
    }
    ok
}

pub async fn warm(with_replay: bool) {
    let legs = if with_replay { 2 } else { 1 };
    if probe(Encoder::Nvenc, legs).await {
        return;
    }
    probe(Encoder::Amf, legs).await;
}

pub async fn build_ladder(
    pref: Option<Encoder>,
    settings: &StreamSettings,
    with_replay: bool,
) -> Vec<Rung> {
    let legs = if with_replay { 2 } else { 1 };
    let mut rungs = Vec::new();

    if pref != Some(Encoder::X264) {
        let order = if pref == Some(Encoder::Amf) {
            [Encoder::Amf, Encoder::Nvenc]
        } else {
            [Encoder::Nvenc, Encoder::Amf]
        };
        for enc in order {
            if probe(enc, legs).await {
                rungs.push(Rung {
                    encoder: enc,
                    framerate: settings.framerate,
                    resolution: settings.resolution,
                });
            }
        }
    }

    let base = Rung {
        encoder: Encoder::X264,
        framerate: settings.framerate,
        resolution: settings.resolution,
    };
    rungs.push(base);
    let half = Rung {
        encoder: Encoder::X264,
        framerate: settings.framerate.min(30),
        resolution: settings.resolution,
    };
    if half != base {
        rungs.push(half);
    }
    let floor = Rung {
        encoder: Encoder::X264,
        framerate: settings.framerate.min(30),
        resolution: 720,
    };
    if floor != half && floor != base {
        rungs.push(floor);
    }
    rungs
}

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

#[cfg(test)]
mod tests;
