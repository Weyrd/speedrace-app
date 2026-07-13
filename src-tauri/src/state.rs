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

// A finish wait the back retried until it work so if mid race outage it finsih still
#[derive(Debug, Clone)]
pub struct PendingFinish {
    pub lobby_id: String,
    pub finishing_time_ms: u64,
    pub run_started_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct PendingRunStarted {
    pub lobby_id: String,
    pub run_start_instant: i64,
}

#[derive(Debug, Clone)]
pub struct PendingSplit {
    pub lobby_id: String,
    pub split_index: u32,
    pub segment_name: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

// A WASM split crossed pre-source-commit -> early start buffered
#[derive(Debug, Clone)]
pub struct BufferedEarlySplit {
    pub lobby_id: String,
    pub split_index: u32,
    pub segment_name: String,
    pub is_final: bool,
}

pub struct GlobalState {
    pub app_state: AppState,
    pub user: Option<UserData>,
    pub ws_status: WsStatus,
    pub lobby: Option<LobbySetup>,
    pub race_start_at: Option<i64>,
    pub clock_offset_ms: i64,
    // penalty
    pub run_start_instant: Option<i64>,
    pub run_active: bool,

    pub run_forfeited: bool,
    pub pending_run_started: Option<PendingRunStarted>,
    pub run_started_retry_running: bool,
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
    pub last_autosplit_reported: Option<(bool, bool, bool)>,
    pub autosplit_source: Option<AutosplitSource>,
    pub wasm_attached: bool,
    pub livesplit_connected: bool,
    pub livesplit_splits_match: Option<bool>,
    pub counter_config: Option<Vec<crate::api::counter_config::CounterConfig>>,
    pub counter_buffers: HashMap<String, CounterBuffer>,
    pub pending_finish: Option<PendingFinish>,
    pub finish_retry_running: bool,
    pub pending_splits: Vec<PendingSplit>,
    pub split_retry_running: bool,
    // Last IGT the WASM reported. start is only (re)captured when it advances (rules out stale-menu re-capture)
    pub wasm_last_igt: Option<i64>,
    pub pending_early_splits: Vec<BufferedEarlySplit>,
    pub stream: Option<crate::stream::StreamSession>,

    pub capture_source: Option<crate::stream::CaptureSource>,
    pub preview: Option<crate::stream::PreviewSession>,
    pub preview_last_jpeg: Option<Vec<u8>>,
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
            run_start_instant: None,
            run_active: false,
            run_forfeited: false,
            pending_run_started: None,
            run_started_retry_running: false,
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
            pending_splits: Vec::new(),
            split_retry_running: false,
            wasm_last_igt: None,
            pending_early_splits: Vec::new(),
            stream: None,
            capture_source: None,
            preview: None,
            preview_last_jpeg: None,
        }
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

// Clear all run-start capture so the next race starts fresh.
pub fn reset_run_start(g: &mut GlobalState) {
    g.run_start_instant = None;
    g.run_active = false;
    g.run_forfeited = false;
    g.pending_run_started = None;
    g.wasm_last_igt = None;
    g.pending_early_splits.clear();
}

pub type SharedState = Arc<Mutex<GlobalState>>;
