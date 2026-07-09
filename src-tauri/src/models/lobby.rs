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
}

impl LobbyStatus {
    pub fn to_app_state(&self) -> AppState {
        match self {
            Self::InProgress => AppState::RaceInProgress,
            Self::Waiting => AppState::StreamSetup,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbySetup {
    // One struct shared by AppEvent::LobbySetup and the lobby/current response.
    pub lobby_id: String,
    pub code: String,
    pub lobby_status: LobbyStatus,
    pub player_status: PlayerStatus,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
    pub max_duration_minutes: u32,
    pub race_start_at: Option<i64>,
    pub expires_at: i64,
    #[serde(default)]
    pub game_id: String,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub category_split_id: Option<String>,
    #[serde(default)]
    pub split_resource_updated_at: Option<String>,
    #[serde(default)]
    pub autosplitter_updated_at: Option<String>,
    #[serde(default)]
    pub counter_config_updated_at: Option<String>,
}

#[derive(Debug, Serialize, Default)]
pub struct AutosplitState {
    pub wasm: bool,
    pub livesplit: bool,
    pub splits_match: Option<bool>,
    pub run_in_progress: bool,
}

#[derive(Debug, Serialize)]
pub struct ClientState {
    pub app_state: AppState,
    pub lobby: Option<LobbySetup>,
    pub autosplit: AutosplitState,
}
