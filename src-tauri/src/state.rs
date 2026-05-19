use crate::auth::token_store::UserData;
use crate::ws::commands::WsCommand;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AppState {
    Unauthenticated,
    Connecting,
    Idle,
    StreamSetup,
    WaitingForStart,
    Racing,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsStatus {
    Connected,
    Connecting,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LobbyInfo {
    pub lobby_id: String,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct LobbyStateSnapshot {
    pub app_state: AppState,
    pub lobby: Option<LobbyInfo>,
    pub race_start_at: Option<String>,
}

pub struct GlobalState {
    pub app_state: AppState,
    pub user: Option<UserData>,
    pub ws_status: WsStatus,
    pub lobby: Option<LobbyInfo>,
    pub race_start_at: Option<String>,
    pub ws_cmd_tx: Option<mpsc::Sender<WsCommand>>,
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            app_state: AppState::Unauthenticated,
            user: None,
            ws_status: WsStatus::Disconnected,
            lobby: None,
            race_start_at: None,
            ws_cmd_tx: None,
        }
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<Mutex<GlobalState>>;
