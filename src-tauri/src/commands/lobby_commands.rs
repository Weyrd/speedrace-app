use crate::api;
use crate::events::WS_PLAYER_RESULT;
use crate::models::{AppState, ClientState};
use crate::state::SharedState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn get_lobby_state(state: State<SharedState>) -> Result<ClientState, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(ClientState {
        app_state: guard.app_state.clone(),
        lobby: guard.lobby.clone(),
    })
}

#[tauri::command]
pub async fn send_player_finished(
    app: AppHandle,
    state: State<'_, SharedState>,
    lobby_id: String,
    finishing_time_ms: u64,
) -> Result<(), String> {
    let result = api::lobby::post_player_finished(&app, &lobby_id, finishing_time_ms).await?;
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::Finished;
        guard.race_start_at = None;
    }
    let _ = app.emit(WS_PLAYER_RESULT, result);
    Ok(())
}

#[tauri::command]
pub async fn send_player_forfeited(
    app: AppHandle,
    state: State<'_, SharedState>,
    lobby_id: String,
) -> Result<(), String> {
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
    guard.app_state = crate::models::AppState::Idle;
    guard.lobby = None;
    Ok(())
}
