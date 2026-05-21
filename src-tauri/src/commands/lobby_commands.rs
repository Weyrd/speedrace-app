use crate::models::ClientState;
use crate::state::SharedState;
use tauri::State;

#[tauri::command]
pub fn get_lobby_state(state: State<SharedState>) -> Result<ClientState, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(ClientState {
        app_state: guard.app_state.clone(),
        lobby: guard.lobby.clone(),
    })
}
