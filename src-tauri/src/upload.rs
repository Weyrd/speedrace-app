use crate::events::UPLOAD_STATUS;
use crate::logging::{mlog, LogCat};
use crate::state::SharedState;
use crate::ws::messages::UploadUnavailableReason;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

// 256 KiB multiple per the resumable protocol
const CHUNK_SIZE: u64 = 8 * 1024 * 1024;
const MAX_RETRIES: u32 = 5;

pub struct UploadSession {
    pub cancel: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadPhase {
    Preparing,
    Uploading,
    Processing,
    Done,
    Failed,
    QuotaExhausted,
    Abandoned,
}

#[derive(Debug, Clone, Serialize)]
pub struct UploadStatusPayload {
    pub state: UploadPhase,
    pub uploaded_bytes: u64,
    pub total_bytes: u64,
    pub message: Option<String>,
}

fn emit(app: &AppHandle, state: UploadPhase, up: u64, total: u64, message: Option<String>) {
    let _ = app.emit(
        UPLOAD_STATUS,
        UploadStatusPayload {
            state,
            uploaded_bytes: up,
            total_bytes: total,
            message,
        },
    );
}

// the back couldnt create a session (quota or Google error)
pub fn emit_unavailable(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: &str,
    reason: UploadUnavailableReason,
) {
    let (replay_base, started_at) = match state.lock() {
        Ok(g) => (g.replay_base.clone(), g.replay_started_at_ms),
        Err(_) => (None, None),
    };
    if let Some(base) = replay_base {
        let _ = crate::settings::save_pending_upload(app, lobby_id, &base, started_at);
    }
    let (phase, msg) = match reason {
        UploadUnavailableReason::QuotaExhausted => (UploadPhase::QuotaExhausted, "quota_exhausted"),
        UploadUnavailableReason::Error => (UploadPhase::Failed, "error"),
    };
    emit(app, phase, 0, 0, Some(msg.to_string()));
}

pub async fn resume_pending(
    app: AppHandle,
    state: SharedState,
    pending: crate::settings::PendingUpload,
) {
    if collect_parts(&pending.replay_base).is_empty() {
        mlog!(LogCat::Api, "[upload] pending replay gone, dropping record");
        crate::settings::clear_pending_upload(&app);
        return;
    }
    match crate::api::lobby::request_upload_ticket_for_resume(&app, &pending.lobby_id).await {
        crate::api::lobby::ResumeTicket::Ready(ticket) => {
            mlog!(
                LogCat::Api,
                "[upload] resuming pending upload for lobby {}",
                pending.lobby_id
            );
            if let Ok(mut guard) = state.lock() {
                guard.replay_base = Some(pending.replay_base.clone());
                guard.replay_started_at_ms = pending.video_started_at_ms;
            }
            spawn(
                &app,
                &state,
                pending.lobby_id,
                ticket.upload_ticket,
                ticket.resumable_url,
            );
        }
        crate::api::lobby::ResumeTicket::NotOwed => {
            mlog!(LogCat::Api, "[upload] pending upload no longer owed");
            crate::settings::clear_pending_upload(&app);
        }
        crate::api::lobby::ResumeTicket::RetryLater => {
            mlog!(LogCat::Api, "[upload] pending upload kept for next start");
        }
    }
}

pub fn spawn(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: String,
    upload_ticket: String,
    resumable_url: String,
) {
    let cancel = Arc::new(AtomicBool::new(false));
    let (replay_base, started_at) = {
        let Ok(mut guard) = state.lock() else { return };
        if guard.upload.is_some() {
            mlog!(LogCat::Api, "[upload] already running, ignoring offer");
            return;
        }
        guard.upload = Some(UploadSession {
            cancel: Arc::clone(&cancel),
        });
        (guard.replay_base.clone(), guard.replay_started_at_ms)
    };
    if let Some(base) = replay_base.as_ref() {
        let _ = crate::settings::save_pending_upload(app, &lobby_id, base, started_at);
    }

    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        let result = run(
            &app,
            &state,
            &lobby_id,
            &upload_ticket,
            &resumable_url,
            &cancel,
        )
        .await;
        if let Err(e) = result {
            if cancel.load(Ordering::SeqCst) {
                mlog!(LogCat::Api, "[upload] abandoned by user");
                // Explicit abandon: the user opted out, don't auto-resume on restart
                crate::settings::clear_pending_upload(&app);
                emit(&app, UploadPhase::Abandoned, 0, 0, None);
            } else {
                // Record stays: a restart retries this upload in the background
                mlog!(LogCat::Api, "[upload] failed: {e}");
                emit(&app, UploadPhase::Failed, 0, 0, Some(e));
            }
        }
        if let Ok(mut g) = state.lock() {
            g.upload = None;
        }
    });
}

async fn run(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: &str,
    upload_ticket: &str,
    resumable_url: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    emit(app, UploadPhase::Preparing, 0, 0, None);

    crate::stream::shutdown(app, state, true).await;

    let (base, video_started_at_ms) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        (guard.replay_base.clone(), guard.replay_started_at_ms)
    };
    let base = base.ok_or("no replay recorded for this race")?;
    let parts = collect_parts(&base);
    if parts.is_empty() {
        return Err(format!("replay file missing: {}", base.display()));
    }

    let file = if parts.len() == 1 {
        parts[0].clone()
    } else {
        concat_parts(&base, &parts).await?
    };

    let total = std::fs::metadata(&file)
        .map_err(|e| format!("replay stat failed: {e}"))?
        .len();
    if total == 0 {
        return Err("replay file is empty".into());
    }

    let video_id = put_chunks(app, &file, total, resumable_url, cancel).await?;
    emit(app, UploadPhase::Processing, total, total, None);

    //the back verifies the video on the channel
    crate::api::lobby::post_vod_complete(
        app,
        lobby_id,
        upload_ticket,
        &video_id,
        video_started_at_ms,
    )
    .await?;

    if crate::settings::load_stream_settings(app).replay_delete_uploaded {
        for p in &parts {
            let _ = std::fs::remove_file(p);
        }
        if file != parts[0] {
            let _ = std::fs::remove_file(&file);
        }
    }

    crate::settings::clear_pending_upload(app);
    mlog!(LogCat::Api, "[upload] done: video_id={video_id}");
    emit(app, UploadPhase::Done, total, total, Some(video_id));
    Ok(())
}

// Base file plus any segments from mid-race ffmpeg restarts
fn collect_parts(base: &Path) -> Vec<PathBuf> {
    let mut parts = Vec::new();
    if base.exists() {
        parts.push(base.to_path_buf());
    }
    let (Some(dir), Some(stem), Some(ext)) = (
        base.parent(),
        base.file_stem().and_then(|s| s.to_str()),
        base.extension().and_then(|s| s.to_str()),
    ) else {
        return parts;
    };
    let prefix = format!("{stem}_pt");
    let suffix = format!(".{ext}");
    let mut numbered: Vec<(u32, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(rest) = name.strip_prefix(&prefix) {
                if let Some(num) = rest.strip_suffix(&suffix) {
                    if let Ok(n) = num.parse::<u32>() {
                        numbered.push((n, e.path()));
                    }
                }
            }
        }
    }
    numbered.sort_by_key(|(n, _)| *n);
    parts.extend(numbered.into_iter().map(|(_, p)| p));
    parts
}

// Lossless join of restart segments (`-f concat -c copy`, seconds not minutes).
async fn concat_parts(base: &Path, parts: &[PathBuf]) -> Result<PathBuf, String> {
    let stem = base
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or("bad replay path")?;
    let dir = base.parent().ok_or("bad replay path")?;
    let list_path = dir.join(format!("{stem}_concat.txt"));
    let out_path = dir.join(format!("{stem}_full.mp4"));

    let list: String = parts
        .iter()
        .map(|p| format!("file '{}'\n", p.display().to_string().replace('\\', "/")))
        .collect();
    std::fs::write(&list_path, list).map_err(|e| format!("concat list write failed: {e}"))?;

    let ffmpeg = crate::stream::ffmpeg_path()?;
    let mut cmd = tokio::process::Command::new(ffmpeg);
    cmd.args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_path)
        .args(["-c", "copy"])
        .arg(&out_path);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
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

enum ProbeState {
    Incomplete(u64),
    Complete(String),
}

// where did Google get to? (used to resume after an error)
async fn probe(client: &reqwest::Client, url: &str, total: u64) -> Result<ProbeState, String> {
    let resp = client
        .put(url)
        .header("Content-Length", 0)
        .header("Content-Range", format!("bytes */{total}"))
        .send()
        .await
        .map_err(|e| format!("resume probe failed: {e}"))?;
    let status = resp.status().as_u16();
    if status == 308 {
        let next = resp
            .headers()
            .get("range")
            .and_then(|v| v.to_str().ok())
            .and_then(|r| r.rsplit('-').next()?.parse::<u64>().ok())
            .map_or(0, |last| last + 1);
        return Ok(ProbeState::Incomplete(next));
    }
    if resp.status().is_success() {
        return Ok(ProbeState::Complete(parse_video_id(resp).await?));
    }
    Err(format!("resume probe returned {status}"))
}

async fn parse_video_id(resp: reqwest::Response) -> Result<String, String> {
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("bad final response: {e}"))?;
    v.get("id")
        .and_then(|x| x.as_str())
        .map(String::from)
        .ok_or_else(|| "no video id in final response".into())
}

async fn put_chunks(
    app: &AppHandle,
    file: &Path,
    total: u64,
    url: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let mut f = tokio::fs::File::open(file)
        .await
        .map_err(|e| format!("replay open failed: {e}"))?;
    let mut offset: u64 = 0;
    let mut retries: u32 = 0;

    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("abandoned".into());
        }

        f.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(|e| format!("seek failed: {e}"))?;
        let len = CHUNK_SIZE.min(total - offset);
        let mut buf = vec![0u8; len as usize];
        f.read_exact(&mut buf)
            .await
            .map_err(|e| format!("read failed: {e}"))?;
        let end = offset + len - 1;

        let resp = client
            .put(url)
            .header("Content-Length", len)
            .header("Content-Range", format!("bytes {offset}-{end}/{total}"))
            .body(buf)
            .send()
            .await;

        match resp {
            Ok(r) if r.status().as_u16() == 308 => {
                offset = r
                    .headers()
                    .get("range")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.rsplit('-').next()?.parse::<u64>().ok())
                    .map_or(end + 1, |last| last + 1);
                retries = 0;
                emit(app, UploadPhase::Uploading, offset, total, None);
            }
            Ok(r) if r.status().is_success() => {
                emit(app, UploadPhase::Uploading, total, total, None);
                return parse_video_id(r).await;
            }
            // Session gone (expired/cancelled): retrying chunks is pointless
            Ok(r) if matches!(r.status().as_u16(), 404 | 410) => {
                return Err(format!("upload session expired ({})", r.status()));
            }
            Ok(r) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(format!("upload failed ({})", r.status()));
                }
                if let Some(id) =
                    backoff_and_resync(&client, url, total, retries, &mut offset).await
                {
                    return Ok(id);
                }
            }
            Err(e) => {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(format!("network error: {e}"));
                }
                if let Some(id) =
                    backoff_and_resync(&client, url, total, retries, &mut offset).await
                {
                    return Ok(id);
                }
            }
        }
    }
}

// Some(video_id) when the probe reveals Google already has the whole file
async fn backoff_and_resync(
    client: &reqwest::Client,
    url: &str,
    total: u64,
    attempt: u32,
    offset: &mut u64,
) -> Option<String> {
    tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt.min(5)))).await;
    match probe(client, url, total).await {
        Ok(ProbeState::Incomplete(next)) => {
            *offset = next;
            None
        }
        Ok(ProbeState::Complete(id)) => Some(id),
        Err(_) => None,
    }
}
