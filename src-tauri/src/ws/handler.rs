use crate::events::{WS_COUNTDOWN, WS_LOBBY_CLOSED, WS_LOBBY_SETUP, WS_RACE_RESULTS};
use crate::models::AppState;
use crate::models::LobbySetup;
use crate::state::SharedState;
use crate::ws::messages::ServerMessage;
use crate::ws_debug;
use tauri::{AppHandle, Emitter};

pub fn handle_message(raw: &str, app: &AppHandle, state: &SharedState) {
    let msg: ServerMessage = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[ws] parse error: {e} — raw: {raw}");
            return;
        }
    };

    ws_debug!("parsed message: {:?}", msg);

    match msg {
        ServerMessage::LobbySetup(payload) => {
            ws_debug!(
                "LobbySetup received: lobby_id={}, game={}",
                payload.lobby_id,
                payload.game_name
            );
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = AppState::StreamSetup;
                guard.lobby = Some(LobbySetup {
                    lobby_id: payload.lobby_id.clone(),
                    stream_key: payload.stream_key.clone(),
                    whip_url: payload.whip_url.clone(),
                    game_name: payload.game_name.clone(),
                    category_name: payload.category_name.clone(),
                });
            }
            let _ = app.emit(WS_LOBBY_SETUP, payload);
        }

        ServerMessage::Countdown(payload) => {
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = AppState::Racing;
                guard.race_start_at = Some(payload.race_start_at.clone());
            }
            let _ = app.emit(WS_COUNTDOWN, payload);
        }

        ServerMessage::RaceResults(payload) => {
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = AppState::Idle;
                guard.lobby = None;
                guard.race_start_at = None;
            }
            let _ = app.emit(WS_RACE_RESULTS, payload.results);
        }

        ServerMessage::LobbyClosed(payload) => {
            ws_debug!(
                "LobbyClosed received: lobby_id={}, reason={}",
                payload.lobby_id,
                payload.reason
            );
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = AppState::Idle;
                guard.lobby = None;
                guard.race_start_at = None;
            }
            let _ = app.emit(WS_LOBBY_CLOSED, payload);
        }

        ServerMessage::Ping => {}
    }
}
