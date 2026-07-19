use super::filler::{validate, with_fillers};
use crate::logging::{mlog, LogCat};
use crate::stream::ffmpeg_command;
use crate::stream::replay::ReplayArtifacts;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

type Segments = Vec<((u32, u32), PathBuf)>;

fn apply_trim_plan(art: &ReplayArtifacts, segments: Segments) -> (Segments, i64) {
    let Some(plan) = art.trim_plan() else {
        mlog!(LogCat::Api, "[upload] no trim plan, uploading untrimmed");
        return (segments, 0);
    };
    let head = (plan.run, plan.first_index);
    let Some(pos) = segments.iter().position(|(k, _)| *k == head) else {
        mlog!(
            LogCat::Api,
            "[upload] trim plan head {head:?} missing, uploading untrimmed"
        );
        return (segments, 0);
    };
    (segments[pos..].to_vec(), plan.head_trim_ms)
}

async fn trim_head(
    app: &AppHandle,
    art: &ReplayArtifacts,
    head: &Path,
    run: u32,
    trim_ms: i64,
) -> Result<PathBuf, String> {
    let s = crate::settings::load_stream_settings(app);
    let fps = s.framerate.max(1);
    let kbps = s.bitrate_kbps.max(500);
    let out = art.head_trim_path();
    let ffmpeg = crate::stream::ffmpeg_path()?;
    let o = ffmpeg_command(&ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-ss"])
        .arg(format!("{:.3}", trim_ms as f64 / 1000.0))
        .arg("-i")
        .arg(head)
        .args(crate::stream::replay_encoder_args(
            art.encoder(run),
            fps,
            kbps,
        ))
        .args(["-c:a", "aac", "-b:a", "160k", "-ar", "48000", "-ac", "2"])
        .args(["-movflags", "+frag_keyframe+empty_moov"])
        .arg(&out)
        .output()
        .await
        .map_err(|e| format!("ffmpeg trim spawn failed: {e}"))?;
    if !o.status.success() || std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0) == 0 {
        return Err(String::from_utf8_lossy(&o.stderr).into_owned());
    }
    Ok(out)
}

pub(super) async fn assemble(app: &AppHandle, base: &Path) -> Result<PathBuf, String> {
    if std::fs::metadata(base).map(|m| m.len()).unwrap_or(0) > 0 {
        return Ok(base.to_path_buf());
    }
    let art = ReplayArtifacts::open(base).ok_or("bad replay path")?;
    let segments = art.segments();
    if segments.is_empty() {
        return Err(format!("replay segments missing: {}", base.display()));
    }
    let (segments, head_trim_ms) = apply_trim_plan(&art, segments);
    let raw: Vec<PathBuf> = segments.iter().map(|(_, p)| p.clone()).collect();

    let mut keyed = segments;
    if head_trim_ms > 0 {
        match trim_head(app, &art, &raw[0], keyed[0].0 .0, head_trim_ms).await {
            Ok(t) => keyed[0].1 = t,
            Err(e) => mlog!(
                LogCat::Api,
                "[upload] head trim failed ({e}), keeping full head"
            ),
        }
    }

    let (spliced, splices, expected) = with_fillers(app, &art, &keyed, head_trim_ms).await;
    let out = concat_parts(app, &art, base, &spliced).await?;
    match validate(&out, &splices, expected, spliced.len()).await {
        Ok(()) => Ok(out),
        Err(e) => {
            mlog!(
                LogCat::Api,
                "[upload] assembly invalid ({e}), rebuilding from untrimmed segments"
            );
            let _ = std::fs::remove_file(&out);
            concat_parts(app, &art, base, &raw).await
        }
    }
}

// Lossless join of restart segments (`-f concat -c copy`, seconds not minutes).
async fn concat_parts(
    app: &AppHandle,
    art: &ReplayArtifacts,
    base: &Path,
    parts: &[PathBuf],
) -> Result<PathBuf, String> {
    let list_path = art.concat_list_path();
    let out_path = base.to_path_buf();

    let list: String = parts
        .iter()
        .map(|p| format!("file '{}'\n", p.display().to_string().replace('\\', "/")))
        .collect();
    std::fs::write(&list_path, list).map_err(|e| format!("concat list write failed: {e}"))?;

    let ffmpeg = crate::stream::ffmpeg_path()?;
    let mut cmd = ffmpeg_command(&ffmpeg);
    cmd.args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path);
    if art.mixed_encoders() {
        let s = crate::settings::load_stream_settings(app);
        mlog!(LogCat::Api, "[upload] mixed encoders, re-encoding the join");
        cmd.args(crate::stream::replay_encoder_args(
            crate::stream::Encoder::X264,
            s.framerate.max(1),
            s.bitrate_kbps.max(500),
        ))
        .args(["-c:a", "aac", "-b:a", "160k", "-ar", "48000", "-ac", "2"]);
    } else {
        cmd.args(["-c", "copy"]);
    }
    cmd.arg(&out_path);
    let out = cmd
        .output()
        .await
        .map_err(|e| format!("ffmpeg concat spawn failed: {e}"))?;
    let _ = std::fs::remove_file(&list_path);
    if !out.status.success() {
        return Err(format!(
            "ffmpeg concat failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(out_path)
}
