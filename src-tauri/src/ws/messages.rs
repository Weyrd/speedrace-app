use crate::models::LobbySetup;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    LobbySetup(Box<LobbySetup>),
    LobbyStart(LobbyStartMsg),
    LobbyClosed(LobbyClosedMsg),
    PlayerResult(PlayerResultPayload),
    EarlyStartWarning { active: bool },
    UploadReady(UploadReadyMsg),
    UploadUnavailable(UploadUnavailableMsg),
    Ping,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UploadReadyMsg {
    pub lobby_id: String,
    pub upload_ticket: String,
    pub resumable_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UploadUnavailableMsg {
    pub lobby_id: String,
    pub reason: UploadUnavailableReason,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum UploadUnavailableReason {
    QuotaExhausted,
    #[serde(other)]
    Error,
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
    #[serde(default)]
    pub start_delay_ms: u64,
    #[serde(default)]
    pub countdown_start_at: Option<i64>,
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
    #[serde(default)]
    pub bingo: Option<BingoResultPayload>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct BingoResultPayload {
    pub outcome: String,
    pub claimed_squares: u32,
    pub total_squares: u32,
    pub duration_ms: i64,
    pub lobby_code: String,
    #[serde(default)]
    pub winners: Vec<String>,
}
