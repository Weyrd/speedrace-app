use crate::api::lobby::fetch_current_lobby;
use crate::auth::oauth::emit_auth_state;
use crate::auth::token_store::TokenStore;
use crate::events::WS_LOBBY_SETUP;
use crate::models::{AppState, AuthStatePayload, AuthUser, LobbyStatus};
use crate::state::SharedState;
use tauri::AppHandle;
use tauri::Emitter;

/// Spawn refresh + WS loops if not already running.
pub fn start_background_loops(app: &AppHandle, state: &SharedState) {
    let should_spawn_refresh = {
        let mut guard = state.lock().unwrap();
        if guard.refresh_loop_running {
            false
        } else {
            guard.refresh_loop_running = true;
            true
        }
    };
    if should_spawn_refresh {
        let app_clone = app.clone();
        let state_clone = state.clone();
        tauri::async_runtime::spawn(async move {
            crate::auth::refresh::token_refresh_loop(app_clone, state_clone).await;
        });
    }

    let should_spawn_ws = {
        let mut guard = state.lock().unwrap();
        if guard.ws_loop_running {
            false
        } else {
            guard.ws_loop_running = true;
            true
        }
    };
    if should_spawn_ws {
        let app_clone = app.clone();
        let state_clone = state.clone();
        tauri::async_runtime::spawn(async move {
            crate::ws::client::ws_connect_loop(app_clone, state_clone).await;
        });
    }
}

/// Try to restore a previous session on startup.
pub async fn restore_session(app: AppHandle, shared_state: SharedState) {
    let store = TokenStore::new(app.clone());

    let stored = match store.load() {
        Some(s) => s,
        None => return,
    };

    let user = if store.is_expired() {
        eprintln!("[startup] access token expired, attempting refresh");
        match crate::auth::refresh::do_refresh(&stored.tokens.refresh_token).await {
            Ok(new_tokens) => {
                if let Err(e) = store.update_tokens(new_tokens) {
                    eprintln!("[startup] failed to persist refreshed tokens: {e}");
                    store.clear().ok();
                    emit_auth_state(&app, AuthStatePayload::Unauthenticated);
                    return;
                }
                stored.user
            }
            Err(e) => {
                eprintln!("[startup] refresh failed (session expired): {e}");
                store.clear().ok();
                emit_auth_state(&app, AuthStatePayload::Unauthenticated);
                return;
            }
        }
    } else {
        stored.user
    };

    // Check if the user is already in a lobby
    let lobby_response = fetch_current_lobby(&app).await;

    {
        let mut guard = shared_state.lock().unwrap();
        if let Some(ref resp) = lobby_response {
            guard.app_state = LobbyStatus::to_app_state(&resp.lobby_status);
            guard.lobby = Some(resp.clone());
            guard.race_start_at = resp.race_start_at.clone();
        } else {
            guard.app_state = AppState::Connecting;
        }
        guard.user = Some(user.clone());
    }

    emit_auth_state(
        &app,
        AuthStatePayload::Authenticated {
            user: AuthUser {
                username: user.username,
            },
        },
    );
    if let Some(ref lobby) = lobby_response {
        let _ = app.emit(WS_LOBBY_SETUP, lobby);
    }

    start_background_loops(&app, &shared_state);
}
