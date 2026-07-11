use crate::settings;
use std::str::FromStr;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

#[tauri::command]
pub fn get_finish_hotkey(app: AppHandle) -> Result<String, String> {
    Ok(settings::load_finish_hotkey(&app))
}

#[tauri::command]
pub fn register_finish_hotkey(app: AppHandle) -> Result<(), String> {
    let accel = settings::load_finish_hotkey(&app);
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(accel.as_str())
        .map_err(|e| format!("failed to register shortcut: {e}"))
}

#[tauri::command]
pub fn unregister_finish_hotkey(app: AppHandle) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_finish_hotkey(app: AppHandle, accelerator: String) -> Result<(), String> {
    Shortcut::from_str(&accelerator).map_err(|e| format!("invalid shortcut: {e}"))?;
    settings::save_finish_hotkey(&app, &accelerator)
}
