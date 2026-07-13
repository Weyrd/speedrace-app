use crate::api;
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::SharedState;
use crate::stream;
use serde::Serialize;
use tauri::Emitter;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::events::APP_STATE;

#[tauri::command]
pub async fn publish_stream(
    lobby_id: String,
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<(), String> {
    stream::publish(&app, &state, &lobby_id).await
}

// Graceful stop from setup/waiting
#[tauri::command]
pub async fn stop_stream(
    lobby_id: String,
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<(), String> {
    stream::shutdown(&app, &state, true).await;

    match api::lobby::post_stream_stopped(&app, &lobby_id).await {
        Ok(()) => mlog!(
            LogCat::Stream,
            "[cmd] stop_stream: back acknowledged ({lobby_id})"
        ),
        Err(e) => mlog!(
            LogCat::Stream,
            "[cmd] stop_stream: POST failed ({lobby_id}): {e}"
        ),
    }

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::StreamSetup;
        guard.race_start_at = None;
    }
    let _ = app.emit(APP_STATE, AppState::StreamSetup);
    stream::preview::ensure_for_phase(&app, &state);
    Ok(())
}

#[derive(Serialize)]
pub struct StreamSettingsDto {
    pub bitrate_kbps: u32,
    pub framerate: u32,
    pub replay_dir: String,
    pub replay_autodelete: bool,
    pub replay_casual: bool,
}

#[tauri::command]
pub fn get_stream_settings(app: AppHandle) -> StreamSettingsDto {
    let s = crate::settings::load_stream_settings(&app);
    StreamSettingsDto {
        bitrate_kbps: s.bitrate_kbps,
        framerate: s.framerate,
        replay_dir: s.replay_dir,
        replay_autodelete: s.replay_autodelete,
        replay_casual: s.replay_casual,
    }
}

#[tauri::command]
pub fn set_stream_settings(
    bitrate_kbps: u32,
    framerate: u32,
    replay_dir: String,
    replay_autodelete: bool,
    replay_casual: bool,
    app: AppHandle,
) -> Result<(), String> {
    // Monitor index is owned by set_capture_source; preserve the stored value
    let monitor_index = crate::settings::load_stream_settings(&app).monitor_index;
    crate::settings::save_stream_settings(
        &app,
        &crate::settings::StoredStreamSettings {
            monitor_index,
            bitrate_kbps,
            framerate,
            replay_dir,
            replay_autodelete,
            replay_casual,
        },
    )
}

#[tauri::command]
pub async fn restart_preview(state: State<'_, SharedState>, app: AppHandle) -> Result<(), String> {
    stream::preview::restart(&app, &state).await
}

#[tauri::command]
pub fn get_capture_source(state: State<'_, SharedState>, app: AppHandle) -> stream::CaptureSource {
    stream::current_source(&app, &state)
}

#[tauri::command]
pub fn set_capture_source(
    source: stream::CaptureSource,
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<(), String> {
    if let stream::CaptureSource::Monitor { index } = source {
        crate::settings::save_monitor_index(&app, index)?;
    }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.capture_source = Some(source);
    Ok(())
}

#[tauri::command]
pub fn open_replay_dir(app: AppHandle) -> Result<(), String> {
    let dir = crate::settings::load_stream_settings(&app).replay_dir;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    app.opener()
        .open_path(dir, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pick_replay_dir(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |folder| {
        let path = folder
            .and_then(|f| f.into_path().ok())
            .map(|p| p.to_string_lossy().into_owned());
        let _ = tx.send(path);
    });
    rx.await.map_err(|e| e.to_string())
}
