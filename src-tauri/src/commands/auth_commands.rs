use crate::auth::oauth::{emit_auth_state, open_browser_login};
use crate::auth::token_store::TokenStore;
use crate::config;
use crate::models::{AppState, AuthStatePayload, LoginError};
use crate::state::SharedState;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn get_app_state(state: State<SharedState>) -> Result<AppState, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.app_state.clone())
}

#[tauri::command]
pub fn open_login(app: AppHandle) -> Result<(), LoginError> {
    open_browser_login(&app)
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
