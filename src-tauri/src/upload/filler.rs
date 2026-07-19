use crate::logging::{mlog, LogCat};
use crate::stream::replay::ReplayArtifacts;
use crate::stream::{ffmpeg_command, NULL_SINK};
use std::path::{Path, PathBuf};
use tauri::AppHandle;

const MAX_FILLER_MS: i64 = 120_000;

async fn video_size(ffmpeg: &Path, seg: &Path) -> Option<(u32, u32)> {
    let out = ffmpeg_command(ffmpeg)
        .args(["-hide_banner", "-i"])
        .arg(seg)
        .output()
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&out.stderr);
    let line = text.lines().find(|l| l.contains("Video:"))?;
    line.split([' ', ',']).find_map(|tok| {
        let (w, h) = tok.split_once('x')?;
        let w: u32 = w.parse().ok()?;
        let h: u32 = h
            .trim_end_matches(|c: char| !c.is_ascii_digit())
            .parse()
            .ok()?;
        (w >= 16 && h >= 16).then_some((w, h))
    })
}

struct FillerSpec {
    fps: u32,
    kbps: u32,
    size: (u32, u32),
}

async fn make_filler(
    ffmpeg: &Path,
    art: &ReplayArtifacts,
    spec: &FillerSpec,
    enc: crate::stream::Encoder,
    gap_ms: i64,
    after_run: u32,
) -> Result<PathBuf, String> {
    let FillerSpec {
        fps,
        kbps,
        size: (w, h),
    } = *spec;
    let out = art.filler_path(after_run);
    let o = ffmpeg_command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(["-f", "lavfi", "-i"])
        .arg(format!("color=c=black:s={w}x{h}:r={fps}"))
        .args(["-f", "lavfi", "-i", "anullsrc=r=48000:cl=stereo"])
        .args(["-t", &format!("{:.3}", gap_ms as f64 / 1000.0)])
        .args(crate::stream::replay_encoder_args(enc, fps, kbps))
        .args(["-c:a", "aac", "-b:a", "160k", "-ar", "48000", "-ac", "2"])
        .args(["-movflags", "+frag_keyframe+empty_moov"])
        .arg(&out)
        .output()
        .await
        .map_err(|e| format!("filler spawn failed: {e}"))?;
    if !o.status.success() || std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(String::from_utf8_lossy(&o.stderr).into_owned());
    }
    Ok(out)
}

// `parts` stays keyed by the ORIGINAL segment: the trimmed head is a different file, so its
// run/index can no longer be recovered from the path once it has been substituted.
pub(super) async fn with_fillers(
    app: &AppHandle,
    art: &ReplayArtifacts,
    parts: &[((u32, u32), PathBuf)],
    head_trim_ms: i64,
) -> (Vec<PathBuf>, Vec<f64>, f64) {
    let plain = || parts.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>();
    let Ok(ffmpeg) = crate::stream::ffmpeg_path() else {
        return (plain(), Vec::new(), 0.0);
    };
    let s = crate::settings::load_stream_settings(app);
    let keys: Vec<(u32, u32)> = parts.iter().map(|(k, _)| *k).collect();
    let durs = art.segment_durations(&keys);
    let spec = video_size(&ffmpeg, &parts[0].1)
        .await
        .map(|size| FillerSpec {
            fps: s.framerate.max(1),
            kbps: s.bitrate_kbps.max(500),
            size,
        });

    let mut out: Vec<PathBuf> = Vec::with_capacity(parts.len());
    let mut splices = Vec::new();
    let mut t = 0.0f64;
    let mut prev: Option<u32> = None;

    for (i, (key, p)) in parts.iter().enumerate() {
        let run = key.0;
        let crossed = prev.filter(|prev_run| *prev_run != run);
        if let Some(prev_run) = crossed {
            if let Some((a, b)) = art.anchor(prev_run).zip(art.anchor(run)) {
                let gap = b.anchor_ms - (a.anchor_ms + a.last_end_ms);
                if gap > MAX_FILLER_MS {
                    mlog!(LogCat::Api, "[upload] gap {gap}ms too large, not filling");
                } else if gap > 0 {
                    match spec.as_ref() {
                        Some(spec) => {
                            let enc = art.encoder(prev_run);
                            match make_filler(&ffmpeg, art, spec, enc, gap, prev_run).await {
                                Ok(f) => {
                                    splices.push(t);
                                    out.push(f);
                                    t += gap as f64 / 1000.0;
                                    splices.push(t);
                                }
                                Err(e) => {
                                    mlog!(LogCat::Api, "[upload] filler r{prev_run} failed: {e}")
                                }
                            }
                        }
                        None => mlog!(LogCat::Api, "[upload] no frame size, skipping filler"),
                    }
                }
            }
        }

        let mut d = durs.get(key).copied().unwrap_or(0.0);
        if i == 0 && head_trim_ms > 0 {
            d = (d - head_trim_ms as f64 / 1000.0).max(0.0);
            if parts.len() > 1 {
                splices.push(t + d);
            }
        }
        t += d;
        out.push(p.clone());
        prev = Some(run);
    }
    let total = if durs.is_empty() { 0.0 } else { t };
    (out, splices, total)
}

pub(super) async fn validate(
    out: &Path,
    splices: &[f64],
    expected_secs: f64,
    pieces: usize,
) -> Result<(), String> {
    let ffmpeg = crate::stream::ffmpeg_path()?;
    if expected_secs > 0.0 {
        let probe = ffmpeg_command(&ffmpeg)
            .args(["-hide_banner", "-i"])
            .arg(out)
            .output()
            .await
            .map_err(|e| format!("probe spawn failed: {e}"))?;
        let text = String::from_utf8_lossy(&probe.stderr);
        let actual = text
            .lines()
            .find_map(|l| l.trim().strip_prefix("Duration: ").map(str::to_string))
            .and_then(|d| {
                let mut it = d.split(',').next()?.split(':');
                let h: f64 = it.next()?.parse().ok()?;
                let m: f64 = it.next()?.parse().ok()?;
                let s: f64 = it.next()?.parse().ok()?;
                Some(h * 3600.0 + m * 60.0 + s)
            });

        // Each joined file rounds up by ~0.03s, so the slack has to grow with the piece
        // count: a 30 min race is ~360 segments and drifts ~11s off the index total.
        let tol = 2.0 + 0.05 * pieces as f64;
        if let Some(actual) = actual {
            if (actual - expected_secs).abs() > tol {
                return Err(format!(
                    "duration {actual:.1}s, expected {expected_secs:.1}s (tol {tol:.1}s)"
                ));
            }
        }
    }

    for t in splices {
        let o = ffmpeg_command(&ffmpeg)
            .args(["-v", "error", "-ss", &format!("{:.3}", (t - 2.0).max(0.0))])
            .args(["-t", "4", "-i"])
            .arg(out)
            .args(["-an", "-vf", "scale=64:-2", "-c:v", "mjpeg", "-f", "mjpeg"])
            .args(["-y", NULL_SINK])
            .output()
            .await
            .map_err(|e| format!("validate spawn failed: {e}"))?;
        let err = String::from_utf8_lossy(&o.stderr);
        if !o.status.success() || !err.trim().is_empty() {
            return Err(format!("splice at {t:.1}s: {}", err.trim()));
        }
    }
    Ok(())
}
