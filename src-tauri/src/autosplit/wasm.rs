use crate::autosplit::timer::MomentumTimer;
use crate::state::SharedState;
use livesplit_auto_splitting::{AutoSplitter, CompiledAutoSplitter, Config, Runtime};
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
        Some(b) => eprintln!("[wasm] cached {} bytes for game {game_id}", b.len()),
        None => eprintln!("[wasm] no autosplitter for game {game_id}"),
    }
    guard.autosplitter_wasm = bytes;
}

// Compile once, then supervise (re-instantiates cheaply on trap). false = broken module → LiveSplit
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
            eprintln!("[wasm] Runtime::new error: {e}");
            return false;
        }
    };
    let compiled = match runtime.compile(&wasm) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[wasm] compile error: {e}");
            return false;
        }
    };

    let Some(splitter) = instantiate(&compiled, &app, &state) else {
        return false;
    };

    eprintln!("[wasm] started");
    tauri::async_runtime::spawn(async move {
        supervise(compiled, splitter, app, state, cancel).await;
    });
    true
}

// A fresh instance clears the permanent trap flag
fn instantiate(
    compiled: &CompiledAutoSplitter,
    app: &AppHandle,
    state: &SharedState,
) -> Option<Arc<AutoSplitter<MomentumTimer>>> {
    let timer = MomentumTimer {
        app: app.clone(),
        state: state.clone(),
    };
    match compiled.instantiate(timer, None, None) {
        Ok(s) => {
            let s = Arc::new(s);
            state.lock().unwrap().autosplitter_runtime = Some(Arc::clone(&s));
            Some(s)
        }
        Err(e) => {
            eprintln!("[wasm] instantiate error: {e}");
            None
        }
    }
}

enum Tick {
    Trapped,
    Ran { attached: bool },
    Busy,
}

async fn supervise(
    compiled: CompiledAutoSplitter,
    mut splitter: Arc<AutoSplitter<MomentumTimer>>,
    app: AppHandle,
    state: SharedState,
    cancel: Arc<AtomicBool>,
) {
    // "connected" = attached to the game process; report only on change.
    let mut last_attached: Option<bool> = None;

    loop {
        let lost = state.lock().unwrap().autosplit_source
            == Some(crate::state::AutosplitSource::LiveSplit);
        if cancel.load(Ordering::SeqCst) || !crate::ws::handler::in_lobby(&state) || lost {
            break;
        }

        // Commit before update() so a split fired this tick goes to the chosen source
        crate::ws::handler::maybe_commit_source(&state);

        let tick_rate = splitter.tick_rate();

        // ExecutionGuard is !Send, so drop it before any await
        let tick = match splitter.try_lock() {
            Some(mut exec) => {
                if exec.update().is_err() {
                    Tick::Trapped
                } else {
                    Tick::Ran {
                        attached: exec.attached_processes().next().is_some(),
                    }
                }
            }
            None => Tick::Busy,
        };

        match tick {
            Tick::Ran { attached } => {
                if last_attached != Some(attached) {
                    last_attached = Some(attached);
                    state.lock().unwrap().wasm_attached = attached;
                    crate::ws::handler::report_autosplit_state(&app, &state).await;
                }
                sleep(tick_rate).await;
            }
            Tick::Trapped => {
                // Trap is permanent for this instance, usually because the game is not running yet
                eprintln!("[wasm] update trapped, re-instantiating");
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

    // Leaving (lobby ended or LiveSplit won): mark detached and report.
    state.lock().unwrap().wasm_attached = false;
    crate::ws::handler::report_autosplit_state(&app, &state).await;
    eprintln!("[wasm] supervisor stopped");
}
