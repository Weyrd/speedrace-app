use crate::auth::oauth::{emit_auth_state, AuthStatePayload};
use crate::auth::token_store::TokenStore;
use crate::config;
use crate::state::{AppState, SharedState, LobbyStateSnapshot};
use crate::ws::commands::WsCommand;
use tauri::{AppHandle, State};

// --- Auth
#[tauri::command]
pub fn get_app_state(state: State<SharedState>) -> Result<AppState, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.app_state.clone())
}

#[tauri::command]
pub fn open_login(app: AppHandle) -> Result<(), String> {
    crate::auth::oauth::open_browser_login(&app)
}

#[tauri::command]
pub fn get_current_user(state: State<SharedState>) -> Result<Option<CurrentUser>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.user.as_ref().map(|u| CurrentUser {
        username: u.username.clone(),
    }))
}

#[derive(serde::Serialize)]
pub struct CurrentUser {
    pub username: String,
}

// clear and return "Unauthenticated" state 
#[tauri::command]
pub async fn logout(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    let refresh_token = TokenStore::new(app.clone()).load()
        .map(|a| a.tokens.refresh_token);
    TokenStore::new(app.clone()).clear()?;

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::Unauthenticated;
        guard.user = None;
        guard.lobby = None;
        guard.race_start_at = None;
        guard.ws_cmd_tx = None;
    }

    emit_auth_state(&app, AuthStatePayload::Unauthenticated);

    if let Some(rt) = refresh_token {
        #[derive(serde::Serialize)]
        struct LogoutRequest { refresh_token: String }

        let _ = reqwest::Client::new()
            .delete(config::api_url(config::AUTH_LOGOUT_PATH))
            .json(&LogoutRequest { refresh_token: rt })
            .send()
            .await;
    }

    Ok(())
}


// Send "StreamReady",  set state to WaitingForStart
#[tauri::command]
pub fn notify_stream_ready(lobby_id: String, state: State<SharedState>) -> Result<(), String> {
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

/// Called by the webview when the user stops the stream.
#[tauri::command]
pub fn notify_stream_stopped(state: State<SharedState>, app: AppHandle) -> Result<(), String> {
    use crate::events::APP_STATE;
    use tauri::Emitter;

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::Idle;
        guard.lobby = None;
        guard.race_start_at = None;
    }

    let _ = app.emit(APP_STATE, AppState::Idle);

    Ok(())
}


#[tauri::command]
pub fn get_lobby_state(state: State<SharedState>) -> Result<LobbyStateSnapshot, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(LobbyStateSnapshot {
        app_state: guard.app_state.clone(),
        lobby: guard.lobby.clone(),
        race_start_at: guard.race_start_at.clone(),
    })
}
 