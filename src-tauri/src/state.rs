use crate::auth::token_store::UserData;
use crate::models::{AppState, LobbySetup, WsStatus};
use crate::ws::commands::WsCommand;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

pub struct GlobalState {
    pub app_state: AppState,
    pub user: Option<UserData>,
    pub ws_status: WsStatus,
    pub lobby: Option<LobbySetup>,
    pub race_start_at: Option<String>,
    pub ws_cmd_tx: Option<mpsc::Sender<WsCommand>>,
    pub refresh_loop_running: bool,
    pub ws_loop_running: bool,
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
            refresh_loop_running: false,
            ws_loop_running: false,
        }
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<Mutex<GlobalState>>;
