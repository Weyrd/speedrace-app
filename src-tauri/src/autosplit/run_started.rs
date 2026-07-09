use crate::api::client::PostOutcome;
use crate::autosplit::now_epoch_ms;
use crate::logging::{mlog, LogCat};
use crate::state::{PendingRunStarted, SharedState};
use std::time::Duration;

// Send when race starts to compute penalty or not
pub fn mark_run_start(app: &tauri::AppHandle, state: &SharedState, instant: i64) {
    let pending = {
        let mut g = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
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
        let mut g = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        g.pending_run_started = Some(pending);
        if g.run_started_retry_running {
            true
        } else {
            g.run_started_retry_running = true;
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
        let g = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        g.pending_run_started.clone()
    } {
        let elapsed_ms = now_epoch_ms() - pending.run_start_instant;
        match crate::api::lobby::submit_run_started(&app, &pending.lobby_id, elapsed_ms).await {
            PostOutcome::Ok(()) | PostOutcome::Rejected => {
                if let Ok(mut g) = state.lock() {
                    g.pending_run_started = None;
                }
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
    if let Ok(mut g) = state.lock() {
        g.run_started_retry_running = false;
    }
}
