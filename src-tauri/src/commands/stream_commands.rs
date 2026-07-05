use crate::api;
use crate::logging::{mlog, LogCat};
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
    lobby_id: String,
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<(), String> {
    mlog!(
        LogCat::Stream,
        "[cmd] send_stream_stopped: POST stream-stopped for lobby {lobby_id}"
    );

    // Best-effort: even if the server is unreachable we still clear local state.
    match api::lobby::post_stream_stopped(&app, &lobby_id).await {
        Ok(()) => mlog!(
            LogCat::Stream,
            "[cmd] send_stream_stopped: back acknowledged (lobby {lobby_id})"
        ),
        Err(e) => mlog!(
            LogCat::Stream,
            "[cmd] send_stream_stopped: POST failed (lobby {lobby_id}): {e}"
        ),
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
