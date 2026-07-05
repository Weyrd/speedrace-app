use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::api::lobby::fetch_current_lobby;
use crate::auth::oauth::emit_auth_state;
use crate::auth::token_store::TokenStore;
use crate::config;
use crate::events::{APP_STATE, WS_LOBBY_SETUP, WS_STATUS};
use crate::logging::{mlog, LogCat};
use crate::models::{AppState, AuthStatePayload, AuthUser, LobbyStatus, WsStatus};
use crate::state::SharedState;

// Server close codes that distinguish an auth rejection from a transient drop.
const CLOSE_AUTH_INVALID: u16 = 4001;
const CLOSE_BANNED: u16 = 4003;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

enum AuthOutcome {
    Ok,
    Invalid,
    Banned,
    Transient,
}

// A single connect+auth attempt; on success carries the live authed stream.
enum AttemptResult {
    Connected(Box<WsStream>),
    Invalid,
    Banned,
    Transient,
}

pub async fn ws_connect_loop(app: AppHandle, state: SharedState) {
    let mut backoff = Duration::from_secs(config::WS_RECONNECT_BASE_SECS);
    let mut transient_failures: u32 = 0;

    loop {
        let token = match TokenStore::new(app.clone()).get_access_token() {
            Some(t) => t,
            None => {
                mlog!(LogCat::Ws, "[ws] no access token, aborting connect loop");
                break;
            }
        };

        match attempt_connection(&app, &state, &token).await {
            AttemptResult::Connected(stream) => {
                transient_failures = 0;
                backoff = Duration::from_secs(config::WS_RECONNECT_BASE_SECS);
                serve_connection(&app, &state, *stream).await;
                // Healthy connection dropped: brief pause, then reconnect.
                emit_ws_status(&app, &state, WsStatus::Disconnected);
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(config::WS_RECONNECT_MAX_SECS));
            }
            AttemptResult::Invalid => {
                mlog!(
                    LogCat::Ws,
                    "[ws] auth rejected (4001), attempting token refresh"
                );
                if refresh_or_logout(&app, &state).await {
                    backoff = Duration::from_secs(config::WS_RECONNECT_BASE_SECS);
                    continue; // reconnect immediately with the refreshed token
                }
                break; // refresh failed -> logged out
            }
            AttemptResult::Banned => {
                mlog!(LogCat::Ws, "[ws] connection refused: banned (4003)");
                emit_app_state(&app, &state, AppState::Banned);
                break;
            }
            AttemptResult::Transient => {
                if register_transient(&app, &state, &mut transient_failures, &mut backoff).await {
                    break;
                }
            }
        }
    }

    if let Ok(mut guard) = state.lock() {
        guard.ws_loop_running = false;
    }
}

pub async fn retry_once(app: AppHandle, state: SharedState) {
    let token = match TokenStore::new(app.clone()).get_access_token() {
        Some(t) => t,
        None => {
            logout(&app, &state, &TokenStore::new(app.clone()));
            return;
        }
    };

    match attempt_connection(&app, &state, &token).await {
        AttemptResult::Connected(stream) => {
            announce_connection(&app, &state).await;
            state.lock().unwrap().ws_loop_running = true;
            let app = app.clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                read_loop(&app, &state, *stream).await;
                emit_ws_status(&app, &state, WsStatus::Disconnected);
                state.lock().unwrap().ws_loop_running = false;
                crate::lifecycle::start_background_loops(&app, &state);
            });
        }
        AttemptResult::Invalid => {
            mlog!(LogCat::Ws, "[ws] retry auth rejected (4001), refreshing");
            if refresh_or_logout(&app, &state).await {
                crate::lifecycle::start_background_loops(&app, &state);
            }
        }
        AttemptResult::Banned => {
            mlog!(LogCat::Ws, "[ws] retry refused: banned (4003)");
            emit_app_state(&app, &state, AppState::Banned);
        }
        AttemptResult::Transient => {
            emit_app_state(&app, &state, AppState::ServerUnavailable);
        }
    }
}

async fn attempt_connection(app: &AppHandle, state: &SharedState, token: &str) -> AttemptResult {
    let url = config::ws_url();
    emit_ws_status(app, state, WsStatus::Connecting);
    mlog!(LogCat::Ws, "[ws] connecting to {}", url);

    let mut ws_stream = match connect_async(&url).await {
        Ok((stream, _)) => stream,
        Err(e) => {
            mlog!(LogCat::Ws, "[ws] connect failed: {e}");
            return AttemptResult::Transient;
        }
    };

    let auth_msg = format!(r#"{{"type":"auth","token":"{}"}}"#, token);
    if ws_stream
        .send(Message::Text(auth_msg.into()))
        .await
        .is_err()
    {
        mlog!(LogCat::Ws, "[ws] failed to send auth message");
        return AttemptResult::Transient;
    }

    match read_auth_outcome(&mut ws_stream).await {
        AuthOutcome::Ok => AttemptResult::Connected(Box::new(ws_stream)),
        AuthOutcome::Invalid => AttemptResult::Invalid,
        AuthOutcome::Banned => AttemptResult::Banned,
        AuthOutcome::Transient => AttemptResult::Transient,
    }
}

async fn serve_connection(app: &AppHandle, state: &SharedState, ws_stream: WsStream) {
    announce_connection(app, state).await;
    read_loop(app, state, ws_stream).await;
}

async fn announce_connection(app: &AppHandle, state: &SharedState) {
    let user = state.lock().unwrap().user.clone();
    if let Some(user) = user {
        emit_auth_state(
            app,
            AuthStatePayload::Authenticated {
                user: AuthUser {
                    username: user.username,
                },
            },
        );
    }

    emit_ws_status(app, state, WsStatus::Connected);
    mlog!(LogCat::Ws, "[ws] connected successfully");

    let current = fetch_current_lobby(app).await;
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
    } else {
        // No active lobby (deleted or expired while disconnected) - reset to Idle
        let mut guard = state.lock().unwrap();
        if guard.app_state != AppState::Unauthenticated {
            guard.app_state = AppState::Idle;
            guard.lobby = None;
            guard.race_start_at = None;
            drop(guard);
            let _ = app.emit(APP_STATE, AppState::Idle);
        }
    }
}

async fn read_loop(app: &AppHandle, state: &SharedState, mut ws_stream: WsStream) {
    loop {
        match ws_stream.next().await {
            Some(Ok(Message::Text(text))) => {
                mlog!(LogCat::Ws, "[ws] received message: {}", text);
                crate::ws::handler::handle_message(&text, app, state);
            }
            Some(Ok(Message::Close(_))) | None => {
                mlog!(LogCat::Ws, "[ws] connection closed by server");
                break;
            }
            Some(Err(e)) => {
                mlog!(LogCat::Ws, "[ws] receive error: {e}");
                break;
            }
            _ => {}
        }
    }
}

// Reads frames until the server's auth verdict is known.
async fn read_auth_outcome<S>(ws_stream: &mut S) -> AuthOutcome
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    loop {
        match ws_stream.next().await {
            // Contract: auth_ok is the first server text on success; any text means we're authed.
            Some(Ok(Message::Text(_))) => return AuthOutcome::Ok,
            Some(Ok(Message::Close(frame))) => {
                let code = frame.map(|f| u16::from(f.code)).unwrap_or(0);
                return match code {
                    CLOSE_AUTH_INVALID => AuthOutcome::Invalid,
                    CLOSE_BANNED => AuthOutcome::Banned,
                    _ => AuthOutcome::Transient,
                };
            }
            Some(Ok(_)) => continue, // ping/pong/binary before the verdict
            Some(Err(_)) | None => return AuthOutcome::Transient,
        }
    }
}

// Counts a transient failure; returns true once the maintenance threshold is hit.
async fn register_transient(
    app: &AppHandle,
    state: &SharedState,
    failures: &mut u32,
    backoff: &mut Duration,
) -> bool {
    *failures += 1;
    emit_ws_status(app, state, WsStatus::Disconnected);
    if *failures >= config::WS_MAX_RETRIES {
        emit_app_state(app, state, AppState::ServerUnavailable);
        return true;
    }
    tokio::time::sleep(*backoff).await;
    *backoff = (*backoff * 2).min(Duration::from_secs(config::WS_RECONNECT_MAX_SECS));
    false
}

// 4001: try one refresh. Returns true to retry with a new token, false if logged out.
async fn refresh_or_logout(app: &AppHandle, state: &SharedState) -> bool {
    let store = TokenStore::new(app.clone());
    let refresh_token = match store.load() {
        Some(a) => a.tokens.refresh_token,
        None => {
            logout(app, state, &store);
            return false;
        }
    };
    match crate::auth::refresh::do_refresh(&refresh_token).await {
        Ok(tokens) => match store.update_tokens(tokens) {
            Ok(()) => true,
            Err(e) => {
                mlog!(LogCat::Ws, "[ws] failed to persist refreshed tokens: {e}");
                logout(app, state, &store);
                false
            }
        },
        Err(e) => {
            mlog!(LogCat::Ws, "[ws] token refresh failed: {e}");
            logout(app, state, &store);
            false
        }
    }
}

fn logout(app: &AppHandle, state: &SharedState, store: &TokenStore) {
    let _ = store.clear();
    {
        let mut guard = state.lock().unwrap();
        guard.app_state = AppState::Unauthenticated;
        guard.user = None;
        guard.lobby = None;
        guard.race_start_at = None;
        guard.pending_finish = None;
    }
    emit_auth_state(app, AuthStatePayload::Unauthenticated);
}

fn emit_app_state(app: &AppHandle, state: &SharedState, app_state: AppState) {
    {
        let mut guard = state.lock().unwrap();
        guard.app_state = app_state.clone();
    }
    let _ = app.emit(APP_STATE, &app_state);
}

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
