use crate::models::ClientState;
use crate::state::SharedState;
use crate::ws::commands::WsCommand;
use tauri::State;

#[tauri::command]
pub fn get_lobby_state(state: State<SharedState>) -> Result<ClientState, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(ClientState {
        app_state: guard.app_state.clone(),
        lobby: guard.lobby.clone(),
    })
}

#[tauri::command]
pub fn send_player_finished(
    state: State<SharedState>,
    lobby_id: String,
    finishing_time_ms: u64,
) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let tx = guard.ws_cmd_tx.as_ref().ok_or("WebSocket not connected")?;
    tx.try_send(WsCommand::PlayerFinished { lobby_id, finishing_time_ms })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn send_player_forfeited(state: State<SharedState>, lobby_id: String) -> Result<(), String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let tx = guard.ws_cmd_tx.as_ref().ok_or("WebSocket not connected")?;
    tx.try_send(WsCommand::PlayerForfeited { lobby_id })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn acknowledge_results(state: State<SharedState>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.app_state = crate::models::AppState::Idle;
    guard.lobby = None;
    Ok(())
}
