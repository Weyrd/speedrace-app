use crate::autosplit::timer::SpeedraceTimer;
use crate::logging::{mlog, LogCat};
use crate::state::SharedState;
use livesplit_auto_splitting::{
    settings, AutoSplitter, CompiledAutoSplitter, Config, Process, Runtime,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::AppHandle;
use tokio::time::{sleep, Duration};

const REINSTANTIATE_DELAY_MS: u64 = 1000;

pub async fn fetch(app: &AppHandle, state: &SharedState, game_id: &str, updated_at: Option<&str>) {
    let bytes = crate::api::autosplitter::fetch_game_autosplitter(app, game_id, updated_at).await;
    let mut guard = state.lock().unwrap();
    match &bytes {
        Some(b) => mlog!(
            LogCat::Wasm,
            "[wasm] cached {} bytes for game {game_id}",
            b.len()
        ),
        None => mlog!(LogCat::Wasm, "[wasm] no autosplitter for game {game_id}"),
    }
    guard.autosplitter_wasm = bytes;
}

pub async fn start(app: AppHandle, state: SharedState, cancel: Arc<AtomicBool>) -> bool {
    let wasm = {
        let g = state.lock().unwrap();
        if g.autosplitter_runtime.is_some() {
            return true; // already running
        }
        g.autosplitter_wasm.clone()
    };
    let Some(wasm) = wasm else {
        return false;
    };

    let mut cfg = Config::default();
    cfg.optimize = true;

    let runtime = match Runtime::new(cfg) {
        Ok(r) => r,
        Err(e) => {
            mlog!(LogCat::Wasm, "[wasm] Runtime::new error: {e}");
            return false;
        }
    };
    let compiled = match runtime.compile(&wasm) {
        Ok(c) => c,
        Err(e) => {
            mlog!(LogCat::Wasm, "[wasm] compile error: {e}");
            return false;
        }
    };

    let Some(splitter) = instantiate(&compiled, &app, &state) else {
        return false;
    };

    mlog!(LogCat::Wasm, "[wasm] started");
    tauri::async_runtime::spawn(async move {
        supervise(compiled, splitter, app, state, cancel).await;
    });
    true
}

fn build_settings_map(state: &SharedState) -> Option<settings::Map> {
    let guard = state.lock().unwrap();
    let xml = guard.split_run.as_ref()?.auto_splitter_settings();
    if xml.is_empty() {
        return None;
    }
    let mut map = settings::Map::new();
    map.insert(
        "autosplitter_settings".into(),
        settings::Value::String(xml.into()),
    );
    Some(map)
}

// A fresh instance clears the permanent trap flag
fn instantiate(
    compiled: &CompiledAutoSplitter,
    app: &AppHandle,
    state: &SharedState,
) -> Option<Arc<AutoSplitter<SpeedraceTimer>>> {
    let timer = SpeedraceTimer {
        app: app.clone(),
        state: state.clone(),
    };
    match compiled.instantiate(timer, build_settings_map(state), None) {
        Ok(s) => {
            let s = Arc::new(s);
            state.lock().unwrap().autosplitter_runtime = Some(Arc::clone(&s));
            Some(s)
        }
        Err(e) => {
            mlog!(LogCat::Wasm, "[wasm] instantiate error: {e}");
            None
        }
    }
}

enum Tick {
    Trapped,
    Ran { attached: bool, pid: Option<u32> },
    Busy,
}

#[allow(clippy::unnecessary_cast)]
#[inline]
fn attached_pid(p: &Process) -> u32 {
    p.pid() as u32
}

fn maybe_switch_source(app: &AppHandle, state: &SharedState, pid: Option<u32>) {
    #[cfg(windows)]
    {
        let Some(pid) = pid else { return };
        let app = app.clone();
        let state = state.clone();
        tauri::async_runtime::spawn(async move {
            crate::stream::auto_select_game_window(&app, &state, pid).await;
        });
    }
    #[cfg(not(windows))]
    {
        let _ = (app, state, pid);
    }
}

async fn supervise(
    compiled: CompiledAutoSplitter,
    mut splitter: Arc<AutoSplitter<SpeedraceTimer>>,
    app: AppHandle,
    state: SharedState,
    cancel: Arc<AtomicBool>,
) {
    // "connected" = attached to the game process; report only on change.
    let mut last_attached: Option<bool> = None;
    // The .lss loads in parallel with startup; push settings once it lands
    let mut settings_pushed = false;

    loop {
        if !settings_pushed {
            if let Some(map) = build_settings_map(&state) {
                splitter.set_settings_map(map);
                settings_pushed = true;
                mlog!(LogCat::Wasm, "[wasm] autosplitter settings pushed");
            }
        }

        let lost = state.lock().unwrap().autosplit_source
            == Some(crate::state::AutosplitSource::LiveSplit);
        if cancel.load(Ordering::SeqCst) || !crate::ws::handler::in_lobby(&state) || lost {
            break;
        }

        // Commit before update() so a split fired this tick goes to the chosen source
        crate::ws::handler::maybe_commit_source(&state);
        // Once committed to WASM at the gun, flush any pre-gun (early-start)
        crate::autosplit::split::flush_early_splits(&app, &state);

        let tick_rate = splitter.tick_rate();

        // ExecutionGuard is !Send, so drop it before any await
        let tick = match splitter.try_lock() {
            Some(mut exec) => {
                if exec.update().is_err() {
                    Tick::Trapped
                } else {
                    let pid = exec.attached_processes().next().map(attached_pid);
                    Tick::Ran {
                        attached: pid.is_some(),
                        pid,
                    }
                }
            }
            None => Tick::Busy,
        };

        match tick {
            Tick::Ran { attached, pid } => {
                if last_attached != Some(attached) {
                    last_attached = Some(attached);
                    state.lock().unwrap().wasm_attached = attached;
                    if attached {
                        maybe_switch_source(&app, &state, pid);
                    }
                    crate::ws::handler::report_autosplit_state(&app, &state).await;
                }
                sleep(tick_rate).await;
            }
            Tick::Trapped => {
                // Trap is permanent for this instance, usually because the game is not running yet
                mlog!(LogCat::Wasm, "[wasm] update trapped, re-instantiating");
                if last_attached != Some(false) {
                    last_attached = Some(false);
                    state.lock().unwrap().wasm_attached = false;
                    crate::ws::handler::report_autosplit_state(&app, &state).await;
                }
                if let Some(s) = instantiate(&compiled, &app, &state) {
                    splitter = s;
                }
                sleep(Duration::from_millis(REINSTANTIATE_DELAY_MS)).await;
            }
            Tick::Busy => sleep(tick_rate).await,
        }
    }

    // Leaving (lobby ended or LiveSplit won): mark detached, unregister so the
    // next lobby's start() doesn't see a stale "already running" handle.
    {
        let mut guard = state.lock().unwrap();
        guard.wasm_attached = false;
        guard.autosplitter_runtime = None;
    }
    crate::ws::handler::report_autosplit_state(&app, &state).await;
    mlog!(LogCat::Wasm, "[wasm] supervisor stopped");
}
