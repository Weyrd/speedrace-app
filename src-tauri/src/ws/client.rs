use std::time::Duration;

use futures_util::StreamExt;
use tauri::AppHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::api::lobby::fetch_current_lobby;
use crate::auth::token_store::TokenStore;
use crate::config;
use crate::models::{AppState, LobbyStatus, WsStatus};
use crate::state::SharedState;
use crate::ws_debug;

pub async fn ws_connect_loop(app: AppHandle, state: SharedState) {
    let mut backoff = Duration::from_secs(config::WS_RECONNECT_BASE_SECS);

    loop {
        // Get a fresh token each reconnect attempt
        let token = match TokenStore::new(app.clone()).get_access_token() {
            Some(t) => t,
            None => {
                eprintln!("[ws] no access token, aborting connect loop");
                return;
            }
        };

        let url = config::ws_url(&token);
        emit_ws_status(&app, &state, WsStatus::Connecting);
        ws_debug!("connecting to {}", url);

        match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                backoff = Duration::from_secs(config::WS_RECONNECT_BASE_SECS); // reset on success
                emit_ws_status(&app, &state, WsStatus::Connected);
                ws_debug!("connected successfully");

                // Fetch current lobby on (re)connect to recover missed messages
                let current = fetch_current_lobby(&app).await;
                if let Some(lobby_resp) = current {
                    let new_app_state;
                    {
                        let mut guard = state.lock().unwrap();

                        guard.app_state = LobbyStatus::to_app_state(&lobby_resp.lobby_status);
                        guard.lobby = Some(lobby_resp.clone());
                        new_app_state = guard.app_state.clone();
                    }
                    let _ = app.emit(APP_STATE, &new_app_state);
                    let _ = app.emit(WS_LOBBY_SETUP, &lobby_resp);
                }

                let mut ws_stream = ws_stream;

                loop {
                    match ws_stream.next().await {
                        Some(Ok(Message::Text(text))) => {
                            ws_debug!("received message: {}", text);
                            crate::ws::handler::handle_message(&text, &app, &state);
                        }
                        Some(Ok(Message::Close(_))) | None => {
                            eprintln!("[ws] connection closed by server");
                            break;
                        }
                        Some(Err(e)) => {
                            eprintln!("[ws] receive error: {e}");
                            break;
                        }
                        _ => {}
                    }
                }
            }

            Err(e) => {
                eprintln!("[ws] connect failed: {e}");
            }
        }

        emit_ws_status(&app, &state, WsStatus::Disconnected);

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(config::WS_RECONNECT_MAX_SECS));
    }
}

use crate::events::{APP_STATE, WS_LOBBY_SETUP, WS_STATUS};
use tauri::Emitter;

pub fn emit_ws_status(app: &AppHandle, state: &SharedState, ws_status: WsStatus) {
    let transitioned_to_idle;
    {
        let mut guard = state.lock().unwrap();
        guard.ws_status = ws_status.clone();
        if ws_status == WsStatus::Connected && guard.app_state == AppState::Connecting {
            guard.app_state = AppState::Idle;
            transitioned_to_idle = true;
        } else {
            transitioned_to_idle = false;
        }
    }
    let _ = app.emit(WS_STATUS, &ws_status);
    if transitioned_to_idle {
        let _ = app.emit(APP_STATE, AppState::Idle);
    }
}
