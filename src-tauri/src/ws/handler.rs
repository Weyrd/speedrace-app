use crate::events::{WS_COUNTDOWN, WS_LOBBY_SETUP, WS_RACE_RESULTS};
use crate::state::{AppState, LobbyInfo, SharedState};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    LobbySetup(LobbySetupMsg),
    Countdown(CountdownMsg),
    RaceResults(RaceResultsMsg),
    Ping,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LobbySetupMsg {
    pub lobby_id: String,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CountdownMsg {
    pub race_start_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RaceResultsMsg {
    // TODO: define the actual shape once we have a sample payload
    pub results: serde_json::Value,
}

pub fn handle_message(raw: &str, app: &AppHandle, state: &SharedState) {
    let msg: ServerMessage = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[ws] parse error: {e} — raw: {raw}");
            return;
        }
    };

    match msg {
        ServerMessage::LobbySetup(payload) => {
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = AppState::StreamSetup;
                guard.lobby = Some(LobbyInfo {
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
                guard.app_state = AppState::Finished;
            }
            let _ = app.emit(WS_RACE_RESULTS, payload.results);
        }

        ServerMessage::Ping => {}
    }
}
