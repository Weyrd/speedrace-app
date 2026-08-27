use crate::events::{
    AUTOSPLIT_PROBE, SPLIT_LOADED, WS_LOBBY_CLOSED, WS_LOBBY_SETUP, WS_LOBBY_START,
    WS_PLAYER_RESULT,
};

#[derive(serde::Serialize, Clone)]
pub struct AutosplitProbePayload {
    pub wasm: bool,
    pub livesplit: bool,
    pub splits_match: Option<bool>,
    // true = run started before the start (early start warning during lobby wait)
    pub run_in_progress: bool,
}
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::{AutosplitSource, LockGlobalState, SharedState};
use crate::ws::messages::ServerMessage;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{AppHandle, Emitter};

pub fn handle_message(raw: &str, app: &AppHandle, state: &SharedState) {
    let msg: ServerMessage = match serde_json::from_str(raw) {
        Ok(m) => m,
        Err(e) => {
            mlog!(LogCat::Ws, "[ws] parse error: {e} - raw: {raw}");
            return;
        }
    };

    mlog!(LogCat::Ws, "[ws] parsed message: {:?}", msg);

    match msg {
        ServerMessage::LobbySetup(payload) => {
            let payload = *payload;
            mlog!(
                LogCat::Ws,
                "[ws] LobbySetup: lobby={} game_id={} cat_id={} split_id={:?} split_updated_at={:?} autosplitter_updated_at={:?}",
                payload.lobby_id,
                payload.game_id,
                payload.category_id,
                payload.category_split_id,
                payload.split_resource_updated_at,
                payload.autosplitter_updated_at,
            );
            mlog!(
                LogCat::Ws,
                "[ws] LobbySetup received: lobby_id={}, game={}",
                payload.lobby_id,
                payload.game_name
            );
            {
                let mut guard = state.lock_state();
                guard.autosplitter_cancel.store(false, Ordering::SeqCst);
                guard.app_state = AppState::StreamSetup;
                guard.lobby = Some(payload.clone());
                guard.pending_finish = None;
                guard.last_autosplit_reported = None;
                guard.autosplit.source = None;
                guard.autosplit.wasm_attached = false;
                guard.autosplit.livesplit_connected = false;
                guard.autosplit.livesplit_splits_match = None;
                guard.counter_buffers.clear();
                guard.pending_splits.clear();
                guard.overlay_recent_splits.clear();
                guard.replay_base = None;
                guard.countdown_start_at_ms = None;
                crate::state::reset_run_start(&mut guard);
            }
            let _ = app.emit(WS_LOBBY_SETUP, payload.clone());
            crate::stream::preview::ensure_for_phase(app, state);
            init_lobby_resources(app, state, &payload);
        }

        ServerMessage::LobbyStart(mut payload) => {
            // start_delay_ms = handicap
            let effective_start = payload.race_start_at + payload.start_delay_ms as i64;
            payload.race_start_at = effective_start;
            if payload.countdown_start_at.is_none() {
                mlog!(
                    LogCat::Ws,
                    "[ws] LobbyStart without countdown_start_at (old back): replay will not be trimmed"
                );
            }
            {
                let mut guard = state.lock_state();
                guard.app_state = AppState::RaceInProgress;
                guard.race_start_at = Some(effective_start);
                guard.countdown_start_at_ms = payload.countdown_start_at;
                mlog!(
                    LogCat::Ws,
                    "[ws] LobbyStart: race_start_at={} start_delay_ms={} countdown_start_at={:?} wasm_cached={}",
                    effective_start,
                    payload.start_delay_ms,
                    payload.countdown_start_at,
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
            mlog!(
                LogCat::Ws,
                "[ws] LobbyClosed received: lobby_id={}, reason={}",
                payload.lobby_id,
                payload.reason
            );
            {
                let mut guard = state.lock_state();
                guard.autosplitter_cancel.store(true, Ordering::SeqCst);
                guard.autosplitter_wasm = None;
                guard.autosplitter_runtime = None;
                guard.probe_running = false;
                guard.livesplit_running = false;
                guard.last_autosplit_reported = None;
                guard.autosplit.source = None;
                guard.autosplit.wasm_attached = false;
                guard.autosplit.livesplit_connected = false;
                guard.autosplit.livesplit_splits_match = None;
                guard.app_state = AppState::Idle;
                guard.lobby = None;
                guard.race_start_at = None;
                guard.pending_finish = None;
                guard.split_run = None;
                guard.current_split_index = 0;
                guard.segment_start_ms = 0;
                guard.counter_config = None;
                guard.counter_buffers.clear();
                guard.pending_splits.clear();
                guard.overlay_recent_splits.clear();
                guard.pending_early_splits.clear();
            }
            crate::stream::shutdown_spawn(app, state);
            let _ = app.emit(WS_LOBBY_CLOSED, payload);
        }

        ServerMessage::PlayerResult(payload) => {
            {
                let mut guard = state.lock_state();
                guard.app_state = AppState::Finished;
                guard.race_start_at = None;
                guard.pending_finish = None;
            }
            crate::stream::shutdown_spawn(app, state);
            let _ = app.emit(WS_PLAYER_RESULT, payload);
        }

        ServerMessage::UploadReady(payload) => {
            mlog!(
                LogCat::Ws,
                "[ws] UploadReady: lobby_id={}",
                payload.lobby_id
            );
            crate::upload::spawn(
                app,
                state,
                payload.lobby_id,
                payload.upload_ticket,
                payload.resumable_url,
            );
        }

        ServerMessage::UploadUnavailable(payload) => {
            mlog!(
                LogCat::Ws,
                "[ws] UploadUnavailable: lobby_id={} reason={:?}",
                payload.lobby_id,
                payload.reason
            );
            crate::upload::emit_unavailable(app, state, &payload.lobby_id, payload.reason);
        }

        ServerMessage::EarlyStartWarning { active } => {
            if active {
                let app = app.clone();
                let state = state.clone();
                tauri::async_runtime::spawn(async move {
                    report_autosplit_state(&app, &state).await;
                });
            }
        }

        ServerMessage::Ping => {}
    }
}

pub fn init_lobby_resources(
    app: &AppHandle,
    state: &SharedState,
    lobby: &crate::models::LobbySetup,
) {
    let Some(category_split_id) = lobby.category_split_id.clone() else {
        return;
    };
    if lobby.split_resource_updated_at.is_none() {
        return;
    }
    {
        let app = app.clone();
        let state = state.clone();
        let updated_at = lobby.split_resource_updated_at.clone();
        tauri::async_runtime::spawn(async move {
            load_split_resource(&app, &state, &category_split_id, updated_at.as_deref()).await;
        });
    }
    let can_probe = {
        let mut guard = state.lock_state();
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
        let counter_updated_at = lobby.counter_config_updated_at.clone();
        tauri::async_runtime::spawn(async move {
            let (_, cfg) = tokio::join!(
                crate::autosplit::wasm::fetch(&app, &state, &game_id, updated_at.as_deref()),
                crate::api::counter_config::fetch_counter_config(
                    &app,
                    &game_id,
                    counter_updated_at.as_deref(),
                )
            );
            if let Some(cfg) = cfg {
                state.lock_state().counter_config = Some(cfg);
            }
            let (has_wasm, cancel) = {
                let g = state.lock_state();
                (
                    g.autosplitter_wasm.is_some(),
                    Arc::clone(&g.autosplitter_cancel),
                )
            };
            if has_wasm {
                crate::autosplit::wasm::start(app.clone(), state.clone(), cancel).await;
            }
            spawn_livesplit_supervisor(&app, &state);
            state.lock_state().probe_running = false;
        });
    }
}

pub(crate) fn resume_lobby_resources(
    app: &AppHandle,
    state: &SharedState,
    lobby: &crate::models::LobbySetup,
) {
    let has_resources = state.lock_state().split_run.is_some();
    if has_resources {
        let app = app.clone();
        let state = state.clone();
        tauri::async_runtime::spawn(async move {
            start_autosplitter(app, state).await;
        });
    } else {
        init_lobby_resources(app, state, lobby);
    }
}

async fn start_autosplitter(app: AppHandle, state: SharedState) {
    let (has_wasm, cancel) = {
        let g = state.lock_state();
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
        state.lock_state().app_state,
        AppState::StreamSetup | AppState::WaitingForStart | AppState::RaceInProgress
    )
}

fn race_clock_started(state: &SharedState) -> bool {
    let guard = state.lock_state();
    let Some(start) = guard.race_start_at else {
        return false;
    };
    guard.server_now_ms() >= start
}

pub(crate) fn maybe_commit_source(state: &SharedState) {
    let mut g = state.lock_state();
    if g.autosplit.source.is_some() {
        return;
    }
    let Some(start) = g.race_start_at else {
        return;
    };
    if g.server_now_ms() < start {
        return;
    }
    if g.autosplit.wasm_attached {
        g.autosplit.source = Some(AutosplitSource::Wasm);
    } else if g.autosplit.livesplit_connected {
        g.autosplit.source = Some(AutosplitSource::LiveSplit);
    }
}

fn spawn_livesplit_supervisor(app: &AppHandle, state: &SharedState) {
    let cancel = {
        let mut guard = state.lock_state();
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
            let mut g = state.lock_state();
            g.livesplit_running = false;
            g.autosplit.livesplit_connected = false;
        }
        report_autosplit_state(&app, &state).await;
    });
}

fn wasm_won(state: &SharedState) -> bool {
    state.lock_state().autosplit.source == Some(AutosplitSource::Wasm)
}

async fn livesplit_supervisor(app: AppHandle, state: SharedState, cancel: Arc<AtomicBool>) {
    use crate::autosplit::tcp::RECONNECT_DELAY_MS;
    use tokio::time::{sleep, Duration};

    let mut ever_connected = false;

    loop {
        if cancel.load(Ordering::SeqCst) || !in_lobby(&state) || wasm_won(&state) {
            break;
        }

        if !ever_connected && race_clock_started(&state) {
            mlog!(
                LogCat::LiveSplit,
                "[livesplit-tcp] not connected by race start, manual finish only"
            );
            break;
        }

        match crate::autosplit::tcp::connect().await {
            Some(stream) => {
                ever_connected = true;
                state.lock_state().autosplit.livesplit_connected = true;
                report_autosplit_state(&app, &state).await;
                crate::autosplit::tcp::poll_loop(
                    stream,
                    app.clone(),
                    state.clone(),
                    Arc::clone(&cancel),
                )
                .await;
                state.lock_state().autosplit.livesplit_connected = false;
                report_autosplit_state(&app, &state).await;
            }
            None => {
                report_autosplit_state(&app, &state).await;
            }
        }

        sleep(Duration::from_millis(RECONNECT_DELAY_MS)).await;
    }
}

pub(crate) fn current_autosplit_probe(state: &SharedState) -> AutosplitProbePayload {
    let g = state.lock_state();
    AutosplitProbePayload {
        wasm: g.autosplit.wasm_attached,
        livesplit: g.autosplit.livesplit_connected,
        splits_match: g.autosplit.livesplit_splits_match,
        run_in_progress: g.run_active,
    }
}

pub(crate) async fn report_autosplit_state(app: &AppHandle, state: &SharedState) {
    let run_active = state.lock_state().run_active;
    report_autosplit_state_inner(app, state, run_active).await;
}

pub(crate) async fn report_autosplit_state_not_running(app: &AppHandle, state: &SharedState) {
    report_autosplit_state_inner(app, state, false).await;
}

async fn report_autosplit_state_inner(app: &AppHandle, state: &SharedState, run_in_progress: bool) {
    let (wasm, livesplit, splits_match, connected, splits_valid) = {
        let g = state.lock_state();
        let connected = match g.autosplit.source {
            Some(AutosplitSource::Wasm) => g.autosplit.wasm_attached,
            Some(AutosplitSource::LiveSplit) => g.autosplit.livesplit_connected,
            None => g.autosplit.wasm_attached || g.autosplit.livesplit_connected,
        };
        let splits_valid = match g.autosplit.source {
            Some(AutosplitSource::Wasm) => true,
            _ => g.autosplit.livesplit_splits_match != Some(false),
        };
        (
            g.autosplit.wasm_attached,
            g.autosplit.livesplit_connected,
            g.autosplit.livesplit_splits_match,
            connected,
            splits_valid,
        )
    };
    let _ = app.emit(
        AUTOSPLIT_PROBE,
        AutosplitProbePayload {
            wasm,
            livesplit,
            splits_match,
            run_in_progress,
        },
    );
    report_autosplit(app, state, connected, splits_valid, run_in_progress).await;
}

async fn report_autosplit(
    app: &AppHandle,
    state: &SharedState,
    connected: bool,
    splits_valid: bool,
    run_in_progress: bool,
) {
    let (lobby_id, should_send) = {
        let guard = state.lock_state();
        let lobby_id = guard.lobby.as_ref().map(|l| l.lobby_id.clone());
        (
            lobby_id,
            guard.last_autosplit_reported != Some((connected, splits_valid, run_in_progress)),
        )
    };
    if !should_send {
        return;
    }
    let Some(lobby_id) = lobby_id else {
        return;
    };
    match crate::api::lobby::post_autosplit_status(
        app,
        &lobby_id,
        connected,
        splits_valid,
        run_in_progress,
    )
    .await
    {
        Ok(()) => {
            state.lock_state().last_autosplit_reported =
                Some((connected, splits_valid, run_in_progress));
        }
        Err(e) => mlog!(LogCat::Autosplit, "[autosplit] report failed: {e}"),
    }
}

async fn load_split_resource(
    app: &AppHandle,
    state: &SharedState,
    category_split_id: &str,
    updated_at: Option<&str>,
) {
    mlog!(
        LogCat::Autosplit,
        "[split] load called: category_split_id={category_split_id} updated_at={updated_at:?}"
    );
    let Some(lss) =
        crate::api::category_split::fetch_split_resource_lss(app, category_split_id, updated_at)
            .await
    else {
        mlog!(
            LogCat::Autosplit,
            "[split] skipped: no lss available (updated_at={updated_at:?})"
        );
        return;
    };
    match livesplit_core::run::parser::composite::parse(lss.as_bytes(), None) {
        Ok(parsed) => {
            let seg_count = parsed.run.len();
            {
                let mut guard = state.lock_state();
                guard.split_run = Some(parsed.run);
                guard.current_split_index = 0;
                guard.segment_start_ms = 0;
            }
            mlog!(LogCat::Autosplit, "[split] loaded: {seg_count} segments");
            let _ = app.emit(SPLIT_LOADED, ());
        }
        Err(e) => mlog!(LogCat::Autosplit, "[split] parse error: {e}"),
    }
}
