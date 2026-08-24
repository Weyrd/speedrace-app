use crate::auth::token_store::UserData;
use crate::counter::CounterBuffer;
use crate::models::{AppState, LobbySetup, WsStatus};
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutosplitSource {
    Wasm,
    LiveSplit,
}

#[derive(Debug, Default)]
pub struct RetryLoops {
    pub run_started: bool,
    pub finish: bool,
    pub split: bool,
}

#[derive(Debug, Default)]
pub struct AutosplitLinks {
    pub source: Option<AutosplitSource>,
    pub wasm_attached: bool,
    pub livesplit_connected: bool,
    pub livesplit_splits_match: Option<bool>,
}

impl AutosplitLinks {
    pub fn splits_are_invalid(&self) -> bool {
        self.source == Some(AutosplitSource::LiveSplit)
            && self.livesplit_splits_match == Some(false)
    }
}

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
    pub elapsed_at_capture_ms: i64,
    pub captured_at: Instant,
}

#[derive(Debug, Clone)]
pub struct PendingSplit {
    pub lobby_id: String,
    pub split_index: u32,
    pub segment_name: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

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
    pub clock_anchor: Option<(Instant, i64)>,
    pub run_start_instant: Option<i64>,
    pub run_active: bool,

    pub run_forfeited: bool,
    pub pending_run_started: Option<PendingRunStarted>,
    pub refresh_loop_running: bool,
    pub ws_loop_running: bool,
    pub ws_gen: u64,
    pub ws_shutdown: Arc<tokio::sync::Notify>,
    pub split_run: Option<livesplit_core::Run>,
    pub current_split_index: u32,
    pub segment_start_ms: u64,
    pub autosplitter_wasm: Option<Vec<u8>>,
    pub autosplitter_runtime: Option<
        Arc<livesplit_auto_splitting::AutoSplitter<crate::autosplit::timer::SpeedraceTimer>>,
    >,
    pub autosplitter_cancel: Arc<AtomicBool>,
    pub probe_running: bool,
    pub livesplit_running: bool,
    pub last_autosplit_reported: Option<(bool, bool, bool)>,
    pub autosplit: AutosplitLinks,
    pub counter_config: Option<Vec<crate::api::counter_config::CounterConfig>>,
    pub counter_buffers: HashMap<String, CounterBuffer>,
    pub pending_finish: Option<PendingFinish>,
    pub pending_splits: Vec<PendingSplit>,
    pub retry: RetryLoops,
    pub wasm_last_igt: Option<i64>,
    pub pending_early_splits: Vec<BufferedEarlySplit>,
    pub stream: Option<crate::stream::StreamSession>,

    pub replay_base: Option<std::path::PathBuf>,
    pub countdown_start_at_ms: Option<i64>,
    pub overlay_recent_splits: Vec<String>,
    pub stream_finalizing: bool,
    pub upload: Option<crate::upload::UploadSession>,

    pub capture_source: Option<crate::stream::CaptureSource>,
    pub preview: Option<crate::stream::PreviewSession>,

    pub preview_starting: bool,
    pub preview_gen: u64,
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
            clock_anchor: None,
            run_start_instant: None,
            run_active: false,
            run_forfeited: false,
            pending_run_started: None,
            refresh_loop_running: false,
            ws_loop_running: false,
            ws_gen: 0,
            ws_shutdown: Arc::new(tokio::sync::Notify::new()),
            split_run: None,
            current_split_index: 0,
            segment_start_ms: 0,
            autosplitter_wasm: None,
            autosplitter_runtime: None,
            autosplitter_cancel: Arc::new(AtomicBool::new(false)),
            probe_running: false,
            livesplit_running: false,
            last_autosplit_reported: None,
            autosplit: AutosplitLinks::default(),
            counter_config: None,
            counter_buffers: HashMap::new(),
            pending_finish: None,
            pending_splits: Vec::new(),
            retry: RetryLoops::default(),
            wasm_last_igt: None,
            pending_early_splits: Vec::new(),
            stream: None,
            replay_base: None,
            countdown_start_at_ms: None,
            overlay_recent_splits: Vec::new(),
            stream_finalizing: false,
            upload: None,
            capture_source: None,
            preview: None,
            preview_starting: false,
            preview_gen: 0,
            preview_last_jpeg: None,
        }
    }
}

impl Default for GlobalState {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalState {
    pub fn set_clock_offset(&mut self, offset_ms: i64) {
        self.clock_offset_ms = offset_ms;
        if self.app_state != AppState::RaceInProgress {
            self.clock_anchor =
                Some((Instant::now(), crate::autosplit::now_epoch_ms() + offset_ms));
        }
    }

    pub fn server_now_ms(&self) -> i64 {
        match &self.clock_anchor {
            Some((instant, epoch_at_anchor)) => {
                epoch_at_anchor.saturating_add(instant.elapsed().as_millis() as i64)
            }
            None => crate::autosplit::now_epoch_ms() + self.clock_offset_ms,
        }
    }
}

pub fn reset_run_start(g: &mut GlobalState) {
    g.run_start_instant = None;
    g.run_active = false;
    g.run_forfeited = false;
    g.pending_run_started = None;
    g.wasm_last_igt = None;
    g.pending_early_splits.clear();
}

pub type SharedState = Arc<Mutex<GlobalState>>;

pub trait LockGlobalState {
    fn lock_state(&self) -> std::sync::MutexGuard<'_, GlobalState>;
}

impl LockGlobalState for Mutex<GlobalState> {
    fn lock_state(&self) -> std::sync::MutexGuard<'_, GlobalState> {
        self.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
