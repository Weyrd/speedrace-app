use crate::events::{
    AUTOSPLIT_PROBE, SPLIT_LOADED, WS_LOBBY_CLOSED, WS_LOBBY_SETUP, WS_LOBBY_START,
    WS_PLAYER_RESULT,
};

#[derive(serde::Serialize, Clone)]
struct AutosplitProbePayload {
    kind: &'static str,
}
use crate::models::AppState;
use crate::state::SharedState;
use crate::ws::messages::ServerMessage;
use crate::ws_debug;
use std::sync::{atomic::Ordering, Arc};
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
                guard.app_state = AppState::StreamSetup;
                guard.lobby = Some(payload.clone());
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

/// Starts split resource load and autosplitter probe loop for a lobby.
/// Safe to call from both the WS handler and the startup restore path.
/// The probe loop is guarded by `probe_running` to prevent duplicate loops.
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
    // Probe loop: only start if not already running
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
            let has_wasm = state.lock().unwrap().autosplitter_wasm.is_some();
            if has_wasm {
                wasm_probe_loop(app, state.clone()).await;
            } else {
                livesplit_probe_loop(app, state.clone()).await;
            }
            state.lock().unwrap().probe_running = false;
        });
    }
}

// wasm then livesplit then nothing; emits "none" if both fail so the UI badge clears
async fn start_autosplitter(app: AppHandle, state: SharedState) {
    let cancel = {
        let guard = state.lock().unwrap();
        guard.autosplitter_cancel.store(false, Ordering::SeqCst);
        Arc::clone(&guard.autosplitter_cancel)
    };

    if crate::autosplit::wasm::start(app.clone(), state.clone(), Arc::clone(&cancel)).await {
        eprintln!("[autosplitter] using WASM");
        return;
    }

    if crate::autosplit::tcp::start(app.clone(), state.clone(), cancel).await {
        eprintln!("[autosplitter] using LiveSplit TCP");
        return;
    }

    eprintln!("[autosplitter] none available — manual finish only");
    let _ = app.emit(AUTOSPLIT_PROBE, AutosplitProbePayload { kind: "none" });
}

// Tries to compile-probe the WASM module up to MAX_WASM_RETRIES times.
// On success emits "wasm"; on exhaustion falls through to LiveSplit probing.
async fn wasm_probe_loop(app: AppHandle, state: SharedState) {
    use crate::autosplit::tcp::{MAX_WASM_RETRIES, PROBE_INTERVAL_SECS};
    use tokio::time::{Duration, sleep};

    for attempt in 0..MAX_WASM_RETRIES {
        {
            let phase = state.lock().unwrap().app_state.clone();
            if phase != crate::models::AppState::StreamSetup
                && phase != crate::models::AppState::WaitingForStart
            {
                return;
            }
        }

        if crate::autosplit::wasm::probe(&state).await {
            let _ = app.emit(AUTOSPLIT_PROBE, AutosplitProbePayload { kind: "wasm" });
            return;
        }

        eprintln!("[probe] WASM compile failed (attempt {}/{MAX_WASM_RETRIES})", attempt + 1);
        sleep(Duration::from_secs(PROBE_INTERVAL_SECS)).await;
    }

    eprintln!("[probe] WASM gave up — falling back to LiveSplit probe");
    livesplit_probe_loop(app, state).await;
}

// Probes LiveSplit TCP immediately then every PROBE_INTERVAL_SECS until the phase changes.
async fn livesplit_probe_loop(app: AppHandle, state: SharedState) {
    use crate::autosplit::tcp::PROBE_INTERVAL_SECS;
    use tokio::time::{Duration, sleep};

    emit_livesplit_probe(&app).await;

    loop {
        sleep(Duration::from_secs(PROBE_INTERVAL_SECS)).await;
        let phase = state.lock().unwrap().app_state.clone();
        if phase != crate::models::AppState::StreamSetup
            && phase != crate::models::AppState::WaitingForStart
        {
            break;
        }
        emit_livesplit_probe(&app).await;
    }
}

async fn emit_livesplit_probe(app: &AppHandle) {
    let kind = if crate::autosplit::tcp::probe().await {
        "livesplit"
    } else {
        "none"
    };
    let _ = app.emit(AUTOSPLIT_PROBE, AutosplitProbePayload { kind });
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
