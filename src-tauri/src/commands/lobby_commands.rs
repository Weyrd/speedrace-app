use crate::api;
use crate::api::client::PostOutcome;
use crate::api::lobby::PlayerResult;
use crate::events::WS_PLAYER_RESULT;
use crate::logging::{mlog, LogCat};
use crate::models::lobby::PlayerStatus;
use crate::models::{AppState, AutosplitState, ClientState};
use crate::state::{LockGlobalState, PendingFinish, SharedState};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

#[tauri::command]
pub fn get_split_segments(state: State<SharedState>) -> Vec<String> {
    let guard = state.lock_state();
    guard
        .split_run
        .as_ref()
        .map(|r| {
            (0..r.len())
                .map(|i| r.segment(i).name().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
pub fn get_autosplit_state(state: State<SharedState>) -> crate::ws::handler::AutosplitProbePayload {
    crate::ws::handler::current_autosplit_probe(&state)
}

#[tauri::command]
pub fn get_lobby_state(state: State<SharedState>) -> Result<ClientState, String> {
    let guard = state.lock_state();
    Ok(ClientState {
        app_state: guard.app_state.clone(),
        lobby: guard.lobby.clone(),
        autosplit: AutosplitState {
            wasm: guard.autosplit.wasm_attached,
            livesplit: guard.autosplit.livesplit_connected,
            splits_match: guard.autosplit.livesplit_splits_match,
            run_in_progress: guard.run_active,
        },
    })
}

pub fn start_durable_finish(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: String,
    finishing_time_ms: u64,
) {
    {
        let mut guard = state.lock_state();
        guard.pending_finish = Some(PendingFinish {
            lobby_id,
            finishing_time_ms,
            run_started_at_ms: guard.run_start_instant,
        });
        if guard.retry.finish {
            return;
        }
        guard.retry.finish = true;
    }
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        durable_finish_loop(app, state).await;
    });
}

async fn durable_finish_loop(app: AppHandle, state: SharedState) {
    let mut backoff = Duration::from_secs(crate::config::WS_RECONNECT_BASE_SECS);
    while let Some(pending) = {
        let g = state.lock_state();
        g.pending_finish.clone()
    } {
        crate::counter::flush_all_counter_buffers(&app, &state, &pending.lobby_id).await;

        match api::lobby::submit_finish(
            &app,
            &pending.lobby_id,
            pending.finishing_time_ms,
            pending.run_started_at_ms,
        )
        .await
        {
            PostOutcome::Ok(result) => {
                finalize_finish(&app, &state, &pending.lobby_id, result);
                break;
            }
            PostOutcome::Rejected => {
                mlog!(
                    LogCat::Api,
                    "[finish] server rejected finish for lobby {}: local time {}ms not confirmed",
                    pending.lobby_id,
                    pending.finishing_time_ms
                );
                let result = PlayerResult {
                    player_status: PlayerStatus::Forfeited,
                    finishing_time_ms: Some(pending.finishing_time_ms),
                    finish_position: None,
                };
                finalize_finish(&app, &state, &pending.lobby_id, result);
                break;
            }
            PostOutcome::Transient => {
                mlog!(
                    LogCat::Api,
                    "[finish] back unreachable, retrying in {backoff:?}"
                );
                tokio::time::sleep(backoff).await;
                backoff =
                    (backoff * 2).min(Duration::from_secs(crate::config::WS_RECONNECT_MAX_SECS));
            }
        }
    }
    state.lock_state().retry.finish = false;
}

fn finalize_finish(app: &AppHandle, state: &SharedState, lobby_id: &str, result: PlayerResult) {
    let username;
    {
        let mut guard = state.lock_state();
        match &guard.pending_finish {
            Some(p) if p.lobby_id == lobby_id => {}
            _ => return,
        }
        guard.pending_finish = None;
        guard.app_state = AppState::Finished;
        guard.race_start_at = None;
        guard.run_start_instant = None;
        guard.autosplitter_cancel.store(true, Ordering::SeqCst);
        username = guard.user.as_ref().map(|u| u.username.clone());
    }
    if let Some(username) = username {
        crate::auth::oauth::emit_auth_state(
            app,
            crate::models::AuthStatePayload::Authenticated {
                user: crate::models::AuthUser { username },
            },
        );
    }
    crate::stream::shutdown_spawn(app, state);
    let _ = app.emit(WS_PLAYER_RESULT, result);
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.request_user_attention(Some(tauri::UserAttentionType::Informational));
    }
}

pub async fn finish_race(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: String,
    finishing_time_ms: u64,
) -> Result<(), String> {
    start_durable_finish(app, state, lobby_id, finishing_time_ms);
    Ok(())
}

#[tauri::command]
pub async fn send_player_finished(
    app: AppHandle,
    state: State<'_, SharedState>,
    lobby_id: String,
    finishing_time_ms: u64,
) -> Result<(), String> {
    finish_race(&app, state.inner(), lobby_id, finishing_time_ms).await
}

#[tauri::command]
pub async fn send_player_forfeited(
    app: AppHandle,
    state: State<'_, SharedState>,
    lobby_id: String,
) -> Result<(), String> {
    crate::counter::flush_all_counter_buffers(&app, state.inner(), &lobby_id).await;
    let result = api::lobby::post_player_forfeited(&app, &lobby_id).await?;
    {
        let mut guard = state.lock_state();
        guard.app_state = AppState::Finished;
        guard.race_start_at = None;
        guard.run_start_instant = None;
    }
    crate::stream::shutdown(&app, state.inner(), true).await;
    let _ = app.emit(WS_PLAYER_RESULT, result);
    Ok(())
}

#[tauri::command]
pub fn acknowledge_results(state: State<SharedState>) -> Result<(), String> {
    let mut guard = state.lock_state();
    if guard.upload.is_some() {
        return Err("upload in progress".into());
    }
    guard.autosplitter_cancel.store(true, Ordering::SeqCst);
    guard.autosplitter_runtime = None;
    guard.app_state = crate::models::AppState::Idle;
    guard.lobby = None;
    guard.split_run = None;
    guard.current_split_index = 0;
    guard.segment_start_ms = 0;
    crate::state::reset_run_start(&mut guard);
    Ok(())
}

#[tauri::command]
pub fn abandon_upload(state: State<SharedState>) -> Result<(), String> {
    let guard = state.lock_state();
    if let Some(u) = guard.upload.as_ref() {
        u.cancel.store(true, Ordering::SeqCst);
    }
    Ok(())
}

#[tauri::command]
pub async fn retry_upload(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    let lobby_id = {
        let guard = state.lock_state();
        if guard.upload.is_some() {
            return Err("upload already running".into());
        }
        guard
            .lobby
            .as_ref()
            .map(|l| l.lobby_id.clone())
            .ok_or("no race to upload for")?
    };
    let ticket = api::lobby::post_request_upload_ticket(&app, &lobby_id).await?;
    crate::upload::spawn(
        &app,
        state.inner(),
        lobby_id,
        ticket.upload_ticket,
        ticket.resumable_url,
    );
    Ok(())
}
