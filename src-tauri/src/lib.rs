mod auth;
mod commands;
mod config;
mod events;
mod state;
mod stream;
mod ws;
mod lobby;

use auth::oauth::{emit_auth_state, AuthStatePayload};
use auth::token_store::TokenStore;
use state::{AppState, GlobalState, SharedState};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shared_state: SharedState = Arc::new(Mutex::new(GlobalState::new()));

    tauri::Builder::default()
        .plugin(
            // force only one instance of app
            tauri_plugin_single_instance::init(|app, _argv, _cwd| {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(shared_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::open_login,
            commands::get_current_user,
            commands::logout,
            commands::notify_stream_ready,
            commands::notify_stream_stopped,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Register deep link in DEV
            #[cfg(debug_assertions)]
            app.deep_link().register("momentum").ok();

            // Deep-link handler
            {
                let app_for_link = app_handle.clone();
                let state_for_link = shared_state.clone();

                app.deep_link().on_open_url(move |event| {
                    let app = app_for_link.clone();
                    let state = state_for_link.clone();
                    let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();

                    tauri::async_runtime::spawn(async move {
                        for url in urls {
                            if url.starts_with(config::AUTH_CALLBACK_PREFIX) {
                                auth::oauth::handle_callback(app.clone(), url, state.clone()).await;
                            }
                        }
                    });
                });
            }

            {
                let app_for_restore = app_handle.clone();
                let state_for_restore = shared_state.clone();

                tauri::async_runtime::spawn(async move {
                    restore_session(app_for_restore, state_for_restore).await;
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Try to restore a previous session on startup.
async fn restore_session(app: AppHandle, shared_state: SharedState) {
    let store = TokenStore::new(app.clone());

    let stored = match store.load() {
        Some(s) => s,
        None => return,
    };

    let user = if store.is_expired() {
        eprintln!("[startup] access token expired, attempting refresh");
        match auth::refresh::do_refresh(&stored.tokens.refresh_token).await {
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
    let current_lobby = lobby::fetch_current_lobby(&app).await;

    {
        let mut guard = shared_state.lock().unwrap();
        guard.app_state = if current_lobby.is_some() {
            AppState::StreamSetup
        } else {
            AppState::Connecting
        };
        guard.user = Some(user.clone());
        guard.lobby = current_lobby;
    }

    emit_auth_state(
        &app,
        AuthStatePayload::Authenticated {
            user: auth::oauth::AuthUser {
                username: user.username,
            },
        },
    );

    // Start background loops
    {
        let app_for_refresh = app.clone();
        tauri::async_runtime::spawn(async move {
            auth::refresh::token_refresh_loop(app_for_refresh).await;
        });
    }
    {
        let app_for_ws = app.clone();
        tauri::async_runtime::spawn(async move {
            ws::client::ws_connect_loop(app_for_ws, shared_state).await;
        });
    }
}