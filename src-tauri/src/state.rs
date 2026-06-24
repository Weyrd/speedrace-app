use crate::auth::token_store::UserData;
use crate::models::{AppState, LobbySetup, WsStatus};
use std::sync::{Arc, Mutex};

pub struct GlobalState {
    pub app_state: AppState,
    pub user: Option<UserData>,
    pub ws_status: WsStatus,
    pub lobby: Option<LobbySetup>,
    pub race_start_at: Option<i64>,
    pub clock_offset_ms: i64,
    pub refresh_loop_running: bool,
    pub ws_loop_running: bool,
    #[allow(dead_code)]
    pub split_run: Option<livesplit_core::Run>,
    #[allow(dead_code)]
    pub current_split_index: u32,
    #[allow(dead_code)]
    pub segment_start_ms: u64,
    // step 2: autosplitter_runtime
}

impl GlobalState {
    pub fn new() -> Self {
        Self {
            app_state: AppState::Unauthenticated,
            user: None,
            ws_status: WsStatus::Disconnected,
            lobby: None,
            race_start_at: None,
            clock_offset_ms: 0,
            refresh_loop_running: false,
            ws_loop_running: false,
            split_run: None,
            current_split_index: 0,
            segment_start_ms: 0,
        }
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<Mutex<GlobalState>>;
