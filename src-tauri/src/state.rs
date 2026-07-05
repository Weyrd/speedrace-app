use crate::auth::token_store::UserData;
use crate::counter::CounterBuffer;
use crate::models::{AppState, LobbySetup, WsStatus};
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosplitSource {
    Wasm,
    LiveSplit,
}

// A finish awaiting confirmation from the back; retried until acked so a mid-race
// backend outage can't lose a result.
#[derive(Debug, Clone)]
pub struct PendingFinish {
    pub lobby_id: String,
    pub finishing_time_ms: u64,
}

pub struct GlobalState {
    pub app_state: AppState,
    pub user: Option<UserData>,
    pub ws_status: WsStatus,
    pub lobby: Option<LobbySetup>,
    pub race_start_at: Option<i64>,
    pub clock_offset_ms: i64,
    pub refresh_loop_running: bool,
    pub ws_loop_running: bool,
    pub split_run: Option<livesplit_core::Run>,
    pub current_split_index: u32,
    pub segment_start_ms: u64,
    pub autosplitter_wasm: Option<Vec<u8>>,
    pub autosplitter_runtime:
        Option<Arc<livesplit_auto_splitting::AutoSplitter<crate::autosplit::timer::MomentumTimer>>>,
    pub autosplitter_cancel: Arc<AtomicBool>,
    pub probe_running: bool,
    pub livesplit_running: bool,
    pub last_autosplit_reported: Option<(bool, bool)>,
    pub autosplit_source: Option<AutosplitSource>,
    pub wasm_attached: bool,
    pub livesplit_connected: bool,
    pub livesplit_splits_match: Option<bool>, // check if right split loaded
    pub counter_config: Option<Vec<crate::api::counter_config::CounterConfig>>,
    pub counter_buffers: HashMap<String, CounterBuffer>,
    pub pending_finish: Option<PendingFinish>,
    pub finish_retry_running: bool,
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
            autosplitter_wasm: None,
            autosplitter_runtime: None,
            autosplitter_cancel: Arc::new(AtomicBool::new(false)),
            probe_running: false,
            livesplit_running: false,
            last_autosplit_reported: None,
            autosplit_source: None,
            wasm_attached: false,
            livesplit_connected: false,
            livesplit_splits_match: None,
            counter_config: None,
            counter_buffers: HashMap::new(),
            pending_finish: None,
            finish_retry_running: false,
        }
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedState = Arc<Mutex<GlobalState>>;
