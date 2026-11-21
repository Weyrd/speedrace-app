use crate::models::LobbySetup;
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
pub enum ServerMessage {
    LobbySetup(Box<LobbySetup>),
    LobbyStart(LobbyStartMsg),
    LobbyClosed(LobbyClosedMsg),
    PlayerResult(PlayerResultPayload),
    EarlyStartWarning {
        active: bool,
    },
    UploadReady(UploadReadyMsg),
    UploadUnavailable(UploadUnavailableMsg),
    AuthOk(AuthOkMsg),
    Ping,
    #[serde(other)]
    Unknown,
}

impl ServerMessage {
    pub fn parse(raw: &str) -> serde_json::Result<Self> {
        #[derive(Deserialize)]
        struct EventOnly {
            event: String,
        }
        serde_json::from_str::<Self>(raw).or_else(|err| {
            let tag: EventOnly = serde_json::from_str(raw)?;
            match serde_json::from_value::<Self>(serde_json::json!({ "event": tag.event })) {
                Ok(Self::Unknown) => Ok(Self::Unknown),
                _ => Err(err),
            }
        })
    }
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct AuthOkMsg {
    #[serde(default)]
    pub protocol_version: u32,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientHello<'a> {
    Auth {
        token: &'a str,
        protocol_version: u32,
    },
}

impl<'a> ClientHello<'a> {
    pub fn auth(token: &'a str) -> Self {
        Self::Auth {
            token,
            protocol_version: PROTOCOL_VERSION,
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
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
    pub race_start_at_ms: i64,
    pub expires_at_ms: i64,
    #[serde(default)]
    pub start_delay_ms: u64,
    #[serde(default)]
    pub countdown_start_at_ms: Option<i64>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<String>,
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
