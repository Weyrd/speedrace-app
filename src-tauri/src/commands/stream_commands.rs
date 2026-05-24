use crate::api;
use crate::models::AppState;
use crate::state::SharedState;
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::events::APP_STATE;

#[tauri::command]
pub async fn send_stream_ready(
    lobby_id: String,
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<(), String> {
    api::lobby::post_stream_ready(&app, &lobby_id).await?;

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::WaitingForStart;
    }

    Ok(())
}

#[tauri::command]
pub async fn send_stream_stopped(
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<(), String> {
    let lobby_id = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.lobby.as_ref().map(|l| l.lobby_id.clone())
    };

    if let Some(lid) = lobby_id {
        // Best-effort if the server is unreachable we still clear local state.
        if let Err(e) = api::lobby::post_stream_stopped(&app, &lid).await {
            eprintln!("[cmd] send_stream_stopped: {e}");
        }
    }

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::Idle;
        guard.lobby = None;
        guard.race_start_at = None;
    }

    let _ = app.emit(APP_STATE, AppState::Idle);

    Ok(())
}
