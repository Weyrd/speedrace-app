use crate::events::{
    AUTOSPLIT_PROBE, SPLIT_LOADED, WS_LOBBY_CLOSED, WS_LOBBY_SETUP, WS_LOBBY_START,
    WS_PLAYER_RESULT,
};

#[derive(serde::Serialize, Clone)]
struct AutosplitProbePayload {
    wasm: bool,
    livesplit: bool,
}
use crate::models::AppState;
use crate::state::{AutosplitSource, SharedState};
use crate::ws::messages::ServerMessage;
use crate::ws_debug;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter};

pub fn handle_message(raw: &str, app: &AppHandle, state: &SharedState) {
    let msg: ServerMessage = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("[ws] parse error: {e} - raw: {raw}");
            return;
        }
    };

    ws_debug!("parsed message: {:?}", msg);

    match msg {
        ServerMessage::LobbySetup(payload) => {
            eprintln!(
                "[ws] LobbySetup: lobby={} game_id={} cat_id={} split_updated_at={:?} autosplitter_updated_at={:?}",
                payload.lobby_id,
                payload.game_id,
                payload.category_id,
                payload.split_resource_updated_at,
                payload.autosplitter_updated_at,
            );
            ws_debug!(
                "LobbySetup received: lobby_id={}, game={}",
                payload.lobby_id,
                payload.game_name
            );
            {
                let mut guard = state.lock().unwrap();
                guard.autosplitter_cancel.store(false, Ordering::SeqCst);
                guard.app_state = AppState::StreamSetup;
                guard.lobby = Some(payload.clone());
                guard.last_autosplit_reported = None;
                guard.autosplit_source = None;
                guard.wasm_attached = false;
                guard.livesplit_connected = false;
            }
            let _ = app.emit(WS_LOBBY_SETUP, payload.clone());
            init_lobby_resources(app, state, &payload);
        }

        ServerMessage::LobbyStart(payload) => {
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = AppState::RaceInProgress;
                guard.race_start_at = Some(payload.race_start_at.clone());
                eprintln!(
                    "[ws] LobbyStart: race_start_at={} wasm_cached={}",
                    payload.race_start_at,
                    guard.autosplitter_wasm.is_some()
                );
            }
            let _ = app.emit(WS_LOBBY_START, payload);
            {
                let app = app.clone();
                let state = state.clone();
                tauri::async_runtime::spawn(async move {
                    start_autosplitter(app, state).await;
                });
            }
        }

        ServerMessage::LobbyClosed(payload) => {
            ws_debug!(
                "LobbyClosed received: lobby_id={}, reason={}",
                payload.lobby_id,
                payload.reason
            );
            {
                let mut guard = state.lock().unwrap();
                guard.autosplitter_cancel.store(true, Ordering::SeqCst);
                guard.autosplitter_wasm = None;
                guard.autosplitter_runtime = None;
                guard.probe_running = false;
                guard.livesplit_running = false;
                guard.last_autosplit_reported = None;
                guard.autosplit_source = None;
                guard.wasm_attached = false;
                guard.livesplit_connected = false;
                guard.app_state = AppState::Idle;
                guard.lobby = None;
                guard.race_start_at = None;
                guard.split_run = None;
                guard.current_split_index = 0;
                guard.segment_start_ms = 0;
            }
            let _ = app.emit(WS_LOBBY_CLOSED, payload);
        }

        ServerMessage::PlayerResult(payload) => {
            {
                let mut guard = state.lock().unwrap();
                guard.app_state = AppState::Finished;
                guard.race_start_at = None;
            }
            let _ = app.emit(WS_PLAYER_RESULT, payload);
        }

        ServerMessage::Ping => {}
    }
}

// Loads the split file and starts the autosplitters for a lobby (WS handler + startup restore)
pub fn init_lobby_resources(
    app: &AppHandle,
    state: &SharedState,
    lobby: &crate::models::LobbySetup,
) {
    if lobby.split_resource_updated_at.is_none() {
        return;
    }
    // Split resource load is idempotent — always respawn
    {
        let app = app.clone();
        let state = state.clone();
        let category_id = lobby.category_id.clone();
        let updated_at = lobby.split_resource_updated_at.clone();
        tauri::async_runtime::spawn(async move {
            load_split_resource(&app, &state, &category_id, updated_at.as_deref()).await;
        });
    }
    // Guard against duplicate startup
    let can_probe = {
        let mut guard = state.lock().unwrap();
        if guard.probe_running {
            false
        } else {
            guard.probe_running = true;
            true
        }
    };
    if can_probe {
        let app = app.clone();
        let state = state.clone();
        let game_id = lobby.game_id.clone();
        let updated_at = lobby.autosplitter_updated_at.clone();
        tauri::async_runtime::spawn(async move {
            crate::autosplit::wasm::fetch(&app, &state, &game_id, updated_at.as_deref()).await;
            let (has_wasm, cancel) = {
                let g = state.lock().unwrap();
                (
                    g.autosplitter_wasm.is_some(),
                    Arc::clone(&g.autosplitter_cancel),
                )
            };
            // Run both in parallel: WASM (if any) and LiveSplit
            if has_wasm {
                crate::autosplit::wasm::start(app.clone(), state.clone(), cancel).await;
            }
            spawn_livesplit_supervisor(&app, &state);
            state.lock().unwrap().probe_running = false;
        });
    }
}

// Both autosplitters normally start during setup and run into the race
async fn start_autosplitter(app: AppHandle, state: SharedState) {
    let (has_wasm, cancel) = {
        let g = state.lock().unwrap();
        (
            g.autosplitter_wasm.is_some(),
            Arc::clone(&g.autosplitter_cancel),
        )
    };
    if has_wasm {
        crate::autosplit::wasm::start(app.clone(), state.clone(), cancel).await;
    }
    spawn_livesplit_supervisor(&app, &state);
}

pub(crate) fn in_lobby(state: &SharedState) -> bool {
    matches!(
        state.lock().unwrap().app_state,
        AppState::StreamSetup | AppState::WaitingForStart | AppState::RaceInProgress
    )
}

fn now_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// True once the synced race clock (race_start_at) has actually started counting.
fn race_clock_started(state: &SharedState) -> bool {
    let guard = state.lock().unwrap();
    let Some(start) = guard.race_start_at else {
        return false;
    };
    now_epoch_ms() + guard.clock_offset_ms >= start
}

// Lock the race's autosplit source once the clock starts: WASM if attached, else LiveSplit
pub(crate) fn maybe_commit_source(state: &SharedState) {
    let mut g = state.lock().unwrap();
    if g.autosplit_source.is_some() {
        return;
    }
    let Some(start) = g.race_start_at else {
        return;
    };
    if now_epoch_ms() + g.clock_offset_ms < start {
        return;
    }
    if g.wasm_attached {
        g.autosplit_source = Some(AutosplitSource::Wasm);
    } else if g.livesplit_connected {
        g.autosplit_source = Some(AutosplitSource::LiveSplit);
    }
}

fn spawn_livesplit_supervisor(app: &AppHandle, state: &SharedState) {
    let cancel = {
        let mut guard = state.lock().unwrap();
        if guard.livesplit_running {
            return;
        }
        guard.livesplit_running = true;
        Arc::clone(&guard.autosplitter_cancel)
    };
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        livesplit_supervisor(app.clone(), state.clone(), cancel).await;
        {
            let mut g = state.lock().unwrap();
            g.livesplit_running = false;
            g.livesplit_connected = false;
        }
        report_autosplit_state(&app, &state).await;
    });
}

fn wasm_won(state: &SharedState) -> bool {
    state.lock().unwrap().autosplit_source == Some(AutosplitSource::Wasm)
}

async fn livesplit_supervisor(app: AppHandle, state: SharedState, cancel: Arc<AtomicBool>) {
    use crate::autosplit::tcp::RECONNECT_DELAY_MS;
    use tokio::time::{sleep, Duration};

    let mut ever_connected = false;

    loop {
        // Stop if the lobby ended, or WASM was locked in as the source for this race.
        if cancel.load(Ordering::SeqCst) || !in_lobby(&state) || wasm_won(&state) {
            break;
        }

        // Give up if LiveSplit never connected once the race has started.
        if !ever_connected && race_clock_started(&state) {
            eprintln!("[livesplit-tcp] not connected by race start, manual finish only");
            break;
        }

        match crate::autosplit::tcp::connect().await {
            Some(stream) => {
                ever_connected = true;
                state.lock().unwrap().livesplit_connected = true;
                report_autosplit_state(&app, &state).await;
                crate::autosplit::tcp::poll_loop(
                    stream,
                    app.clone(),
                    state.clone(),
                    Arc::clone(&cancel),
                )
                .await;
                state.lock().unwrap().livesplit_connected = false;
                report_autosplit_state(&app, &state).await;
            }
            None => {
                report_autosplit_state(&app, &state).await;
            }
        }

        sleep(Duration::from_millis(RECONNECT_DELAY_MS)).await;
    }
}

// Emits per-source badges to the UI and the single "connected" signal the back gates on
// connected = either source pre-commit, else only the committed source's health
pub(crate) async fn report_autosplit_state(app: &AppHandle, state: &SharedState) {
    let (wasm, livesplit, connected) = {
        let g = state.lock().unwrap();
        let connected = match g.autosplit_source {
            Some(AutosplitSource::Wasm) => g.wasm_attached,
            Some(AutosplitSource::LiveSplit) => g.livesplit_connected,
            None => g.wasm_attached || g.livesplit_connected,
        };
        (g.wasm_attached, g.livesplit_connected, connected)
    };
    let _ = app.emit(AUTOSPLIT_PROBE, AutosplitProbePayload { wasm, livesplit });
    report_autosplit(app, state, connected).await;
}

// POST to the back only when the autosplit connection state changes
async fn report_autosplit(app: &AppHandle, state: &SharedState, connected: bool) {
    let (lobby_id, should_send) = {
        let guard = state.lock().unwrap();
        let lobby_id = guard.lobby.as_ref().map(|l| l.lobby_id.clone());
        (lobby_id, guard.last_autosplit_reported != Some(connected))
    };
    if !should_send {
        return;
    }
    let Some(lobby_id) = lobby_id else {
        return;
    };
    match crate::api::lobby::post_autosplit_status(app, &lobby_id, connected).await {
        Ok(()) => {
            state.lock().unwrap().last_autosplit_reported = Some(connected);
        }
        Err(e) => eprintln!("[autosplit] report failed: {e}"),
    }
}

async fn load_split_resource(
    app: &AppHandle,
    state: &SharedState,
    category_id: &str,
    updated_at: Option<&str>,
) {
    eprintln!("[split] load called: category_id={category_id} updated_at={updated_at:?}");
    let Some(lss) =
        crate::api::category_split::fetch_category_split_lss(app, category_id, updated_at).await
    else {
        eprintln!("[split] skipped: no lss available (updated_at={updated_at:?})");
        return;
    };
    match livesplit_core::run::parser::composite::parse(lss.as_bytes(), None) {
        Ok(parsed) => {
            let seg_count = parsed.run.len();
            {
                let mut guard = state.lock().unwrap();
                guard.split_run = Some(parsed.run);
                guard.current_split_index = 0;
                guard.segment_start_ms = 0;
            }
            eprintln!("[split] loaded: {seg_count} segments");
            let _ = app.emit(SPLIT_LOADED, ());
        }
        Err(e) => eprintln!("[split] parse error: {e}"),
    }
}
