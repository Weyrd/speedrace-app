use crate::api;
use crate::events::WS_PLAYER_RESULT;
use crate::models::{AppState, AutosplitState, ClientState};
use crate::state::SharedState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn get_split_segments(state: State<SharedState>) -> Vec<String> {
    let guard = state.lock().unwrap();
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
pub fn get_current_split_index(state: State<SharedState>) -> u32 {
    state.lock().unwrap().current_split_index
}

#[tauri::command]
pub fn get_lobby_state(state: State<SharedState>) -> Result<ClientState, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(ClientState {
        app_state: guard.app_state.clone(),
        lobby: guard.lobby.clone(),
        autosplit: AutosplitState {
            wasm: guard.wasm_attached,
            livesplit: guard.livesplit_connected,
        },
    })
}

pub async fn finish_race(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: String,
    finishing_time_ms: u64,
) -> Result<(), String> {
    crate::counter::flush_all_counter_buffers(app, state, &lobby_id).await;
    let result = api::lobby::post_player_finished(app, &lobby_id, finishing_time_ms).await?;
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::Finished;
        guard.race_start_at = None;
    }
    let _ = app.emit(WS_PLAYER_RESULT, result);
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
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::Finished;
        guard.race_start_at = None;
    }
    let _ = app.emit(WS_PLAYER_RESULT, result);
    Ok(())
}

#[tauri::command]
pub fn acknowledge_results(state: State<SharedState>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.autosplitter_cancel.store(true, Ordering::SeqCst);
    guard.autosplitter_runtime = None;
    guard.app_state = crate::models::AppState::Idle;
    guard.lobby = None;
    guard.split_run = None;
    guard.current_split_index = 0;
    guard.segment_start_ms = 0;
    Ok(())
}
