use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "momentum_settings.json";
const FINISH_HOTKEY_KEY: &str = "finish_hotkey";

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
    store.set(FINISH_HOTKEY_KEY, serde_json::Value::String(accel.to_string()));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
