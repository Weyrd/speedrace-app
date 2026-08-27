use crate::api::client::PostOutcome;
use crate::autosplit::now_epoch_ms;
use crate::logging::{mlog, LogCat};
use crate::state::{LockGlobalState, PendingRunStarted, SharedState};
use std::time::Duration;

pub fn mark_run_start(app: &tauri::AppHandle, state: &SharedState, instant: i64) {
    let pending = {
        let mut g = state.lock_state();
        if g.run_start_instant.is_some() {
            return;
        }
        g.run_start_instant = Some(instant);
        let Some(lobby) = g.lobby.as_ref() else {
            return;
        };
        let lobby_id = lobby.lobby_id.clone();
        g.run_active = true;
        PendingRunStarted {
            lobby_id,
            run_start_instant: instant,
            elapsed_at_capture_ms: (now_epoch_ms() - instant).max(0),
            captured_at: std::time::Instant::now(),
        }
    };
    mlog!(
        LogCat::Autosplit,
        "[run-started] signaling run_start_instant={}",
        pending.run_start_instant
    );
    start_durable_run_started(app, state, pending);
}

fn start_durable_run_started(
    app: &tauri::AppHandle,
    state: &SharedState,
    pending: PendingRunStarted,
) {
    let already_running = {
        let mut g = state.lock_state();
        g.pending_run_started = Some(pending);
        if g.retry.run_started {
            true
        } else {
            g.retry.run_started = true;
            false
        }
    };
    let app = app.clone();
    let state = state.clone();
    if already_running {
        tauri::async_runtime::spawn(async move {
            crate::ws::handler::report_autosplit_state(&app, &state).await;
        });
        return;
    }
    tauri::async_runtime::spawn(async move {
        crate::ws::handler::report_autosplit_state(&app, &state).await;
        durable_run_started_loop(app, state).await;
    });
}

async fn durable_run_started_loop(app: tauri::AppHandle, state: SharedState) {
    let mut backoff = Duration::from_secs(crate::config::WS_RECONNECT_BASE_SECS);
    while let Some(pending) = {
        let g = state.lock_state();
        g.pending_run_started.clone()
    } {
        let elapsed_ms =
            pending.elapsed_at_capture_ms + pending.captured_at.elapsed().as_millis() as i64;
        match crate::api::lobby::submit_run_started(&app, &pending.lobby_id, elapsed_ms).await {
            PostOutcome::Ok(()) | PostOutcome::Rejected => {
                state.lock_state().pending_run_started = None;
                break;
            }
            PostOutcome::Transient => {
                mlog!(
                    LogCat::Api,
                    "[run-started] back unreachable, retrying in {backoff:?}"
                );
                tokio::time::sleep(backoff).await;
                backoff =
                    (backoff * 2).min(Duration::from_secs(crate::config::WS_RECONNECT_MAX_SECS));
            }
        }
    }
    state.lock_state().retry.run_started = false;
}
