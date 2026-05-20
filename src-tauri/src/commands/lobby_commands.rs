use crate::models::LobbyStateSnapshot;
use crate::state::SharedState;
use tauri::State;

#[tauri::command]
pub fn get_lobby_state(state: State<SharedState>) -> Result<LobbyStateSnapshot, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(LobbyStateSnapshot {
        app_state: guard.app_state.clone(),
        lobby: guard.lobby.clone(),
        race_start_at: guard.race_start_at.clone(),
    })
}
