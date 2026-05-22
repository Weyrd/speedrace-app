use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tauri::AppHandle;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::auth::token_store::TokenStore;
use crate::config;
use crate::models::{AppState, LobbyStatus, WsStatus};
use crate::state::SharedState;
use crate::ws::commands::WsCommand;
use crate::ws::messages::LobbySetupMsg;
use crate::ws_debug;

const CMD_CHANNEL_SIZE: usize = 32;

pub async fn ws_connect_loop(app: AppHandle, state: SharedState) {
    let (tx, mut rx) = mpsc::channel::<WsCommand>(CMD_CHANNEL_SIZE);

    // Register the sender so Tauri commands can reach us
    {
        let mut guard = state.lock().unwrap();
        guard.ws_cmd_tx = Some(tx);
    }

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
                let current = crate::api::lobby::fetch_current_lobby(&app).await;
                if let Some(lobby_resp) = current {
                    let new_app_state;
                    {
                        let mut guard = state.lock().unwrap();
                        let status = LobbyStatus::from_player_status(lobby_resp.player_status.as_ref());
                        guard.app_state = status.to_app_state();
                        guard.lobby = Some(lobby_resp.lobby.clone());
                        guard.race_start_at = lobby_resp.race_start_at.clone();
                        new_app_state = guard.app_state.clone();
                    }
                    let _ = app.emit(APP_STATE, &new_app_state);
                    let _ = app.emit(
                        WS_LOBBY_SETUP,
                        LobbySetupMsg {
                            lobby_id: lobby_resp.lobby.lobby_id,
                            stream_key: lobby_resp.lobby.stream_key,
                            whip_url: lobby_resp.lobby.whip_url,
                            game_name: lobby_resp.lobby.game_name,
                            category_name: lobby_resp.lobby.category_name,
                        },
                    );
                }

                let (mut write, mut read) = ws_stream.split();

                loop {
                    tokio::select! {
                        // Incoming message from server
                        msg = read.next() => {
                            match msg {
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

                        cmd = rx.recv() => {
                            match cmd {
                                Some(WsCommand::Disconnect) | None => {
                                    let _ = write.send(Message::Close(None)).await;
                                    {
                                        let mut guard = state.lock().unwrap();
                                        guard.ws_cmd_tx = None;
                                    }
                                    emit_ws_status(&app, &state, WsStatus::Disconnected);
                                    return; // exit the loop entirely
                                }
                                Some(cmd) => {
                                    let json = cmd.to_json().expect("non-Disconnect has json");
                                    if let Err(e) = write.send(Message::Text(json.into())).await {
                                        eprintln!("[ws] send error: {e}");
                                        break;
                                    }
                                }
                            }
                        }
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


use crate::events::{WS_STATUS, APP_STATE, WS_LOBBY_SETUP};
use tauri::Emitter;


pub fn emit_ws_status(app: &AppHandle, state: &SharedState, status: WsStatus) {
    let transitioned_to_idle;
    {
        let mut guard = state.lock().unwrap();
        guard.ws_status = status.clone();
        if status == WsStatus::Connected && guard.app_state == AppState::Connecting {
            guard.app_state = AppState::Idle;
            transitioned_to_idle = true;
        } else {
            transitioned_to_idle = false;
        }
    }
    let _ = app.emit(WS_STATUS, &status);
    if transitioned_to_idle {
        let _ = app.emit(APP_STATE, AppState::Idle);
    }
}