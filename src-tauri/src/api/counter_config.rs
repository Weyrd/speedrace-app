use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::config;

use super::cache::{cache_paths, CacheKind};
use super::client::authed_get_json;

// Plain serde enums (no rename_all) to match the back's PascalCase wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterMode {
    Total,
    PerSplit,
    Timeline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CounterCadence {
    Instant,
    PerSplit,
    EndOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterConfig {
    pub counter_name: String,
    pub enabled: bool,
    pub mode: CounterMode,
    pub cadence: CounterCadence,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub display_order: i32,
}

pub async fn fetch_counter_config(
    app: &AppHandle,
    game_id: &str,
    payload_updated_at: Option<&str>,
) -> Option<Vec<CounterConfig>> {
    let payload_updated_at = payload_updated_at?;

    let paths = cache_paths(app, CacheKind::CounterConfig, game_id)?;
    let cache_path = paths.content;
    let stamp_path = paths.stamp;

    if cache_path.exists() {
        if let Ok(cached_stamp) = std::fs::read_to_string(&stamp_path) {
            if cached_stamp.trim() == payload_updated_at {
                if let Ok(bytes) = std::fs::read(&cache_path) {
                    if let Ok(cfg) = serde_json::from_slice::<Vec<CounterConfig>>(&bytes) {
                        return Some(cfg);
                    }
                }
            }
        }
    }

    let path = config::game_counters_path(game_id);
    let config = authed_get_json::<Vec<CounterConfig>>(app, &path, "counters").await?;

    if let Ok(bytes) = serde_json::to_vec(&config) {
        if let Some(parent) = cache_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = std::fs::write(&cache_path, &bytes) {
            eprintln!("[counters] cache write error: {e}");
        }
        let _ = std::fs::write(&stamp_path, payload_updated_at);
    }

    Some(config)
}
