use crate::models::LobbySetup;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    LobbySetup(Box<LobbySetup>), //  lobbySetup donc pas de typeMsg (match WS AppEvent::LobbySetup ET get lobby/current response pour n'avoir qu'une struct)
    LobbyStart(LobbyStartMsg),
    LobbyClosed(LobbyClosedMsg),
    PlayerResult(PlayerResultPayload),
    EarlyStartWarning { active: bool },
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
    pub expires_at: i64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerResultPayload {
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub username: String,
    pub player_status: String,
    pub finishing_time_ms: Option<i64>,
    pub finish_position: Option<i32>,
}
