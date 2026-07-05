use tauri::AppHandle;

use crate::config;
use crate::logging::{mlog, LogCat};

use super::cache::{cache_paths, CacheKind};
use super::client::authed_get_bytes;

#[allow(dead_code)]
pub async fn fetch_game_autosplitter(
    app: &AppHandle,
    game_id: &str,
    payload_updated_at: Option<&str>,
) -> Option<Vec<u8>> {
    let payload_updated_at = payload_updated_at?;

    let paths = cache_paths(app, CacheKind::Autosplitter, game_id)?;
    let cache_path = paths.content;
    let stamp_path = paths.stamp;

    if cache_path.exists() {
        if let Ok(cached_stamp) = std::fs::read_to_string(&stamp_path) {
            if cached_stamp.trim() == payload_updated_at {
                return std::fs::read(&cache_path)
                    .map_err(|e| mlog!(LogCat::Wasm, "[autosplitter] cache read error: {e}"))
                    .ok();
            }
        }
    }

    let path = config::game_autosplitter_download_path(game_id);
    let bytes = authed_get_bytes(app, &path, "autosplitter").await?;

    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&cache_path, &bytes) {
        mlog!(LogCat::Wasm, "[autosplitter] cache write error: {e}");
    }
    let _ = std::fs::write(&stamp_path, payload_updated_at);

    Some(bytes)
}
