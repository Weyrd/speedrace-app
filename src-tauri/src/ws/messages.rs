use crate::models::LobbySetup;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    LobbySetup(LobbySetup),
    LobbyStart(LobbyStartMsg),
    LobbyClosed(LobbyClosedMsg),
    PlayerResult(PlayerResultPayload),
    Ping,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LobbyClosedMsg {
    pub lobby_id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LobbyStartMsg {
    pub race_start_at: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerResultPayload {
    pub user_id: String,
    pub username: String,
    pub player_status: String,
    pub finishing_time_ms: Option<i64>,
    pub finish_position: Option<i32>,
}
