use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "momentum_settings.json";
const FINISH_HOTKEY_KEY: &str = "finish_hotkey";
const CLOCK_OFFSET_KEY: &str = "clock_offset_ms";
const CLOCK_SYNCED_AT_KEY: &str = "clock_synced_at";

pub const DEFAULT_FINISH_HOTKEY: &str = "CmdOrCtrl+Shift+F";

pub fn load_finish_hotkey(app: &AppHandle) -> String {
    app.store(STORE_PATH)
        .ok()
        .and_then(|store| store.get(FINISH_HOTKEY_KEY))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| DEFAULT_FINISH_HOTKEY.to_string())
}

pub fn save_finish_hotkey(app: &AppHandle, accel: &str) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    store.set(
        FINISH_HOTKEY_KEY,
        serde_json::Value::String(accel.to_string()),
    );
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

// (offset_ms, synced_at_ms); None when never measured
pub fn load_clock_offset(app: &AppHandle) -> Option<(i64, i64)> {
    let store = app.store(STORE_PATH).ok()?;
    let offset = store.get(CLOCK_OFFSET_KEY)?.as_i64()?;
    let synced_at = store.get(CLOCK_SYNCED_AT_KEY)?.as_i64()?;
    Some((offset, synced_at))
}

pub fn save_clock_offset(app: &AppHandle, offset_ms: i64, synced_at: i64) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    store.set(CLOCK_OFFSET_KEY, serde_json::Value::from(offset_ms));
    store.set(CLOCK_SYNCED_AT_KEY, serde_json::Value::from(synced_at));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
