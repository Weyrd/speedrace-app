use crate::api::lobby::fetch_current_lobby;
use crate::auth::oauth::emit_auth_state;
use crate::auth::token_store::TokenStore;
use crate::events::{APP_STATE, WS_LOBBY_SETUP};
use crate::logging::{mlog, LogCat};
use crate::models::{AppState, AuthStatePayload, AuthUser, LobbySetup, LobbyStatus, PlayerStatus};
use crate::state::SharedState;
use tauri::AppHandle;
use tauri::Emitter;

pub async fn fetch_and_apply_current_lobby(
    app: &AppHandle,
    state: &SharedState,
) -> Option<(LobbySetup, bool)> {
    let current = fetch_current_lobby(app).await;
    match current {
        Some(lobby_resp) => {
            let player_done = matches!(
                lobby_resp.player_status,
                PlayerStatus::Finished | PlayerStatus::Forfeited
            );
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = if player_done {
                    AppState::Finished
                } else {
                    LobbyStatus::to_app_state(&lobby_resp.lobby_status)
                };
                guard.race_start_at = lobby_resp.race_start_at;
                guard.lobby = Some(lobby_resp.clone());
            }
            Some((lobby_resp, player_done))
        }
        None => {
            let mut guard = state.lock().unwrap();
            if guard.app_state != AppState::Unauthenticated {
                guard.app_state = AppState::Idle;
                guard.lobby = None;
                guard.race_start_at = None;
                drop(guard);
                let _ = app.emit(APP_STATE, AppState::Idle);
            }
            None
        }
    }
}

pub fn broadcast_lobby_restore(
    app: &AppHandle,
    state: &SharedState,
    lobby: &LobbySetup,
    player_done: bool,
) {
    let app_state = state.lock().unwrap().app_state.clone();
    let _ = app.emit(APP_STATE, &app_state);
    let _ = app.emit(WS_LOBBY_SETUP, lobby);
    crate::stream::preview::ensure_for_phase(app, state);
    if !player_done {
        crate::ws::handler::resume_lobby_resources(app, state, lobby);
    }
}

pub async fn sync_current_lobby(app: &AppHandle, state: &SharedState) -> bool {
    match fetch_and_apply_current_lobby(app, state).await {
        Some((lobby, player_done)) => {
            broadcast_lobby_restore(app, state, &lobby, player_done);
            true
        }
        None => false,
    }
}

pub fn start_background_loops(app: &AppHandle, state: &SharedState) -> bool {
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
    should_spawn_ws
}

pub async fn restore_session(app: AppHandle, shared_state: SharedState) {
    let store = TokenStore::new(app.clone());

    let stored = match store.load() {
        Some(s) => s,
        None => return,
    };

    let user = if store.is_expired() {
        mlog!(
            LogCat::Lifecycle,
            "[startup] access token expired, attempting refresh"
        );
        match crate::auth::refresh::do_refresh(&stored.tokens.refresh_token).await {
            Ok(new_tokens) => {
                if let Err(e) = store.update_tokens(new_tokens) {
                    mlog!(
                        LogCat::Lifecycle,
                        "[startup] failed to persist refreshed tokens: {e}"
                    );
                    store.clear().ok();
                    emit_auth_state(&app, AuthStatePayload::Unauthenticated);
                    return;
                }
                stored.user
            }
            Err(e) => {
                mlog!(
                    LogCat::Lifecycle,
                    "[startup] refresh failed (session expired): {e}"
                );
                store.clear().ok();
                emit_auth_state(&app, AuthStatePayload::Unauthenticated);
                return;
            }
        }
    } else {
        stored.user
    };

    {
        let mut guard = shared_state.lock().unwrap();
        guard.app_state = AppState::Connecting;
        guard.user = Some(user.clone());
    }

    let restored = fetch_and_apply_current_lobby(&app, &shared_state).await;

    emit_auth_state(
        &app,
        AuthStatePayload::Authenticated {
            user: AuthUser {
                username: user.username,
            },
        },
    );

    match restored {
        Some((lobby, player_done)) => {
            broadcast_lobby_restore(&app, &shared_state, &lobby, player_done);
        }
        None => {
            if let Some(pending) = crate::settings::load_pending_upload(&app) {
                mlog!(
                    LogCat::Lifecycle,
                    "[startup] pending upload found for lobby {}",
                    pending.lobby_id
                );
                let app_clone = app.clone();
                let state_clone = shared_state.clone();
                tauri::async_runtime::spawn(async move {
                    crate::upload::resume_pending(app_clone, state_clone, pending).await;
                });
            }
        }
    }

    start_background_loops(&app, &shared_state);
}
