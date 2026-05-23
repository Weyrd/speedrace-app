use crate::models::AppState;
use crate::state::SharedState;
use crate::ws::commands::WsCommand;
use tauri::Emitter;
use tauri::{AppHandle, State};

use crate::events::APP_STATE;

#[tauri::command]
//When stream ready -> WS into AppState::WaitingForStart
pub fn send_stream_ready(lobby_id: String, state: State<SharedState>) -> Result<(), String> {
    let sender = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.ws_cmd_tx.clone()
    };

    if let Some(tx) = sender {
        let _ = tx.try_send(WsCommand::StreamReady { lobby_id });
    }

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::WaitingForStart;
    }

    Ok(())
}

#[tauri::command]
//When stream stopped (either by finishing or forfeiting) -> WS into AppState::Idle and clear lobby info
pub fn send_stream_stopped(state: State<SharedState>, app: AppHandle) -> Result<(), String> {
    let (sender, lobby_id) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        let tx = guard.ws_cmd_tx.clone();
        let lid = guard.lobby.as_ref().map(|l| l.lobby_id.clone());
        (tx, lid)
    };

    if let (Some(tx), Some(lid)) = (sender, lobby_id) {
        let _ = tx.try_send(WsCommand::StreamStopped { lobby_id: lid });
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
