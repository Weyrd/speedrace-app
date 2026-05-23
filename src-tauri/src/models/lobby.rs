use serde::{Deserialize, Serialize};

use super::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerStatus {
    Preparing,
    InProgress,
    Finished,
    Forfeited,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LobbyStatus {
    Waiting,
    InProgress,
    Finished,
}

impl LobbyStatus {
    pub fn to_app_state(&self) -> AppState {
        match self {
            Self::InProgress => AppState::RaceInProgress,
            Self::Finished => AppState::Finished,
            Self::Waiting => AppState::StreamSetup, //ou WaitingForStart
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbySetup {
    // match AppEvent::LobbySetup ET get lobby/current response pour n'avoir qu'une struct
    pub lobby_id: String,
    pub code: String,
    pub lobby_status: LobbyStatus,
    pub player_status: PlayerStatus,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
    pub race_start_at: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ClientState {
    pub app_state: AppState,
    pub lobby: Option<LobbySetup>,
}
