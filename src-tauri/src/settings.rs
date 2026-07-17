use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "speedrace_settings.json";
const FINISH_HOTKEY_KEY: &str = "finish_hotkey";
const CLOCK_OFFSET_KEY: &str = "clock_offset_ms";
const CLOCK_SYNCED_AT_KEY: &str = "clock_synced_at";
const STREAM_MONITOR_KEY: &str = "stream_monitor_index";
const STREAM_BITRATE_KEY: &str = "stream_bitrate_kbps";
const STREAM_FRAMERATE_KEY: &str = "stream_framerate";
const STREAM_REPLAY_DIR_KEY: &str = "stream_replay_dir";
const STREAM_REPLAY_AUTODELETE_KEY: &str = "stream_replay_autodelete";
const STREAM_REPLAY_CASUAL_KEY: &str = "stream_replay_casual";
const STREAM_REPLAY_DELETE_UPLOADED_KEY: &str = "stream_replay_delete_uploaded";
const PENDING_UPLOAD_KEY: &str = "pending_upload";

pub const DEFAULT_FINISH_HOTKEY: &str = "CmdOrCtrl+Shift+F";
pub const DEFAULT_STREAM_BITRATE_KBPS: u32 = 2000;
pub const DEFAULT_STREAM_FRAMERATE: u32 = 60;
pub const DEFAULT_REPLAY_AUTODELETE: bool = true;
pub const DEFAULT_REPLAY_CASUAL: bool = false;
pub const DEFAULT_REPLAY_DELETE_UPLOADED: bool = false;

pub const REPLAY_RETENTION_DAYS: u64 = 7;

pub struct StoredStreamSettings {
    pub monitor_index: u32,
    pub bitrate_kbps: u32,
    pub framerate: u32,
    pub replay_dir: String,
    pub replay_autodelete: bool,
    pub replay_casual: bool,
    pub replay_delete_uploaded: bool,
}

// Videos\Speedrace
pub fn default_replay_dir(app: &AppHandle) -> String {
    let base = app
        .path()
        .video_dir()
        .or_else(|_| app.path().app_data_dir())
        .map(|p| p.join("Speedrace"));
    base.map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "Speedrace".to_string())
}

// A VOD still need to be uploaded -> auto restart
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PendingUpload {
    pub lobby_id: String,
    pub replay_base: std::path::PathBuf,
    // clock of when started the race on the video to align with other player
    #[serde(default)]
    pub video_started_at_ms: Option<i64>,
}

pub fn save_pending_upload(
    app: &AppHandle,
    lobby_id: &str,
    replay_base: &std::path::Path,
    video_started_at_ms: Option<i64>,
) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    let value = serde_json::to_value(PendingUpload {
        lobby_id: lobby_id.to_string(),
        replay_base: replay_base.to_path_buf(),
        video_started_at_ms,
    })
    .map_err(|e| e.to_string())?;
    store.set(PENDING_UPLOAD_KEY, value);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_pending_upload(app: &AppHandle) -> Option<PendingUpload> {
    let store = app.store(STORE_PATH).ok()?;
    serde_json::from_value(store.get(PENDING_UPLOAD_KEY)?).ok()
}

pub fn clear_pending_upload(app: &AppHandle) {
    if let Ok(store) = app.store(STORE_PATH) {
        store.delete(PENDING_UPLOAD_KEY);
        let _ = store.save();
    }
}

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

pub fn save_monitor_index(app: &AppHandle, index: u32) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    store.set(STREAM_MONITOR_KEY, serde_json::Value::from(index));
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_stream_settings(app: &AppHandle) -> StoredStreamSettings {
    let store = app.store(STORE_PATH).ok();
    let get_u32 = |k: &str, d: u32| {
        store
            .as_ref()
            .and_then(|s| s.get(k))
            .and_then(|v| v.as_u64())
            .map(|n| n as u32)
            .unwrap_or(d)
    };
    let replay_dir = store
        .as_ref()
        .and_then(|s| s.get(STREAM_REPLAY_DIR_KEY))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| default_replay_dir(app));
    let replay_autodelete = store
        .as_ref()
        .and_then(|s| s.get(STREAM_REPLAY_AUTODELETE_KEY))
        .and_then(|v| v.as_bool())
        .unwrap_or(DEFAULT_REPLAY_AUTODELETE);
    let replay_casual = store
        .as_ref()
        .and_then(|s| s.get(STREAM_REPLAY_CASUAL_KEY))
        .and_then(|v| v.as_bool())
        .unwrap_or(DEFAULT_REPLAY_CASUAL);
    let replay_delete_uploaded = store
        .as_ref()
        .and_then(|s| s.get(STREAM_REPLAY_DELETE_UPLOADED_KEY))
        .and_then(|v| v.as_bool())
        .unwrap_or(DEFAULT_REPLAY_DELETE_UPLOADED);
    StoredStreamSettings {
        monitor_index: get_u32(STREAM_MONITOR_KEY, 0),
        bitrate_kbps: get_u32(STREAM_BITRATE_KEY, DEFAULT_STREAM_BITRATE_KBPS),
        framerate: get_u32(STREAM_FRAMERATE_KEY, DEFAULT_STREAM_FRAMERATE),
        replay_dir,
        replay_autodelete,
        replay_casual,
        replay_delete_uploaded,
    }
}

pub fn save_stream_settings(app: &AppHandle, s: &StoredStreamSettings) -> Result<(), String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    store.set(STREAM_MONITOR_KEY, serde_json::Value::from(s.monitor_index));
    store.set(STREAM_BITRATE_KEY, serde_json::Value::from(s.bitrate_kbps));
    store.set(STREAM_FRAMERATE_KEY, serde_json::Value::from(s.framerate));
    store.set(
        STREAM_REPLAY_DIR_KEY,
        serde_json::Value::from(s.replay_dir.clone()),
    );
    store.set(
        STREAM_REPLAY_AUTODELETE_KEY,
        serde_json::Value::from(s.replay_autodelete),
    );
    store.set(
        STREAM_REPLAY_CASUAL_KEY,
        serde_json::Value::from(s.replay_casual),
    );
    store.set(
        STREAM_REPLAY_DELETE_UPLOADED_KEY,
        serde_json::Value::from(s.replay_delete_uploaded),
    );
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
