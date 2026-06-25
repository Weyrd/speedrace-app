use crate::state::SharedState;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::AppHandle;

pub async fn fetch(app: &AppHandle, state: &SharedState, game_id: &str, updated_at: Option<&str>) {
    let bytes = crate::api::autosplitter::fetch_game_autosplitter(app, game_id, updated_at).await;
    let mut guard = state.lock().unwrap();
    match &bytes {
        Some(b) => eprintln!("[wasm] cached {} bytes for game {game_id}", b.len()),
        None => eprintln!("[wasm] no autosplitter for game {game_id}"),
    }
    guard.autosplitter_wasm = bytes;
}

pub async fn start(app: AppHandle, state: SharedState, cancel: Arc<AtomicBool>) -> bool {
    let wasm = {
        let guard = state.lock().unwrap();
        guard.autosplitter_wasm.clone()
    };
    let Some(wasm) = wasm else {
        return false;
    };

    let timer = crate::autosplit::timer::MomentumTimer {
        app: app.clone(),
        state: state.clone(),
    };

    let mut cfg = livesplit_auto_splitting::Config::default();
    cfg.optimize = true;

    let runtime = match livesplit_auto_splitting::Runtime::new(cfg) {
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

    let splitter = match compiled.instantiate(timer, None, None) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[wasm] instantiate error: {e}");
            return false;
        }
    };

    let splitter = Arc::new(splitter);

    let (cancel_tick, splitter_tick) = {
        let mut guard = state.lock().unwrap();
        guard.autosplitter_runtime = Some(Arc::clone(&splitter));
        (cancel, Arc::clone(&splitter))
    };

    eprintln!("[wasm] started");

    tauri::async_runtime::spawn(async move {
        loop {
            if cancel_tick.load(Ordering::SeqCst) {
                break;
            }

            let tick_rate = splitter_tick.tick_rate();
            tokio::time::sleep(tick_rate).await;

            if cancel_tick.load(Ordering::SeqCst) {
                break;
            }

            if let Some(mut exec) = splitter_tick.try_lock() {
                if let Err(e) = exec.update() {
                    eprintln!("[wasm] update error: {e}");
                    break;
                }
            }
        }
        eprintln!("[wasm] tick loop stopped");
    });

    true
}
