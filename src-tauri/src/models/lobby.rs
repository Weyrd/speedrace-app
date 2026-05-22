use serde::{Deserialize, Serialize};

use super::race::PlayerStatus;
use super::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyStatus {
    InProgress,
    WaitingForStart,
    Finished,
    Other,
}

impl LobbyStatus {
    pub fn from_player_status(status: Option<&PlayerStatus>) -> Self {
        match status {
            Some(PlayerStatus::RaceInProgress) => Self::InProgress,
            Some(PlayerStatus::Ready) => Self::WaitingForStart,
            Some(PlayerStatus::Finished) | Some(PlayerStatus::Forfeited) => Self::Finished,
            _ => Self::Other,
        }
    }

    pub fn to_app_state(&self) -> AppState {
        match self {
            Self::InProgress => AppState::RaceInProgress,
            Self::WaitingForStart => AppState::WaitingForStart,
            Self::Finished => AppState::Finished,
            Self::Other => AppState::StreamSetup,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbySetup {
    pub lobby_id: String,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ClientState {
    pub app_state: AppState,
    pub lobby: Option<LobbySetup>,
}
