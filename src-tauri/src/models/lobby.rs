use serde::{Deserialize, Serialize};

use super::AppState;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LobbyStatus {
    InProgress,
    WaitingForStart,
    Other,
}

impl LobbyStatus {
    pub fn from_opt(s: Option<&str>) -> Self {
        match s {
            Some("InProgress") => Self::InProgress,
            Some("WaitingForStart") => Self::WaitingForStart,
            _ => Self::Other,
        }
    }

    pub fn to_app_state(&self) -> AppState {
        match self {
            Self::InProgress => AppState::Racing,
            Self::WaitingForStart => AppState::WaitingForStart,
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
