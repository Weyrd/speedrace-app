mod assemble;
mod filler;
mod resumable;

use crate::events::UPLOAD_STATUS;
use crate::logging::{mlog, LogCat};
use crate::state::SharedState;
use crate::stream::replay::ReplayArtifacts;
use crate::ws::messages::UploadUnavailableReason;
use assemble::assemble;
use resumable::put_chunks;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};

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
    let replay_base = match state.lock() {
        Ok(g) => g.replay_base.clone(),
        Err(_) => None,
    };
    if let Some(base) = replay_base {
        let _ = crate::settings::save_pending_upload(app, lobby_id, &base);
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
    let assembled = std::fs::metadata(&pending.replay_base)
        .map(|m| m.len())
        .unwrap_or(0)
        > 0;
    let no_segments =
        ReplayArtifacts::open(&pending.replay_base).is_none_or(|a| a.segments().is_empty());
    if !assembled && no_segments {
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
    let replay_base = {
        let Ok(mut guard) = state.lock() else { return };
        if guard.upload.is_some() {
            mlog!(LogCat::Api, "[upload] already running, ignoring offer");
            return;
        }
        guard.upload = Some(UploadSession {
            cancel: Arc::clone(&cancel),
        });
        guard.replay_base.clone()
    };
    if let Some(base) = replay_base.as_ref() {
        let _ = crate::settings::save_pending_upload(app, &lobby_id, base);
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
                crate::settings::clear_pending_upload(&app);
                emit(&app, UploadPhase::Abandoned, 0, 0, None);
            } else {
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

    let base = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.replay_base.clone()
    };
    let base = base.ok_or("no replay recorded for this race")?;
    let file = assemble(app, &base).await?;

    let total = std::fs::metadata(&file)
        .map_err(|e| format!("replay stat failed: {e}"))?
        .len();
    if total == 0 {
        return Err("replay file is empty".into());
    }

    let video_id = put_chunks(app, &file, total, resumable_url, cancel).await?;
    emit(app, UploadPhase::Processing, total, total, None);

    //the back verifies the video on the channel
    crate::api::lobby::post_vod_complete(app, lobby_id, upload_ticket, &video_id).await?;

    if let Some(a) = ReplayArtifacts::open(&base) {
        a.discard();
    }
    if crate::settings::load_stream_settings(app).replay_delete_uploaded {
        let _ = std::fs::remove_file(&file);
    }

    crate::settings::clear_pending_upload(app);
    mlog!(LogCat::Api, "[upload] done: video_id={video_id}");
    emit(app, UploadPhase::Done, total, total, Some(video_id));
    Ok(())
}
