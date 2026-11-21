use reqwest::header::{ETAG, IF_NONE_MATCH};
use reqwest::StatusCode;
use tauri::AppHandle;

use crate::config;
use crate::logging::{mlog, LogCat};

use super::cache::{cache_paths, CacheKind};
use super::client::ApiClient;

pub async fn fetch_game_autosplitter(
    app: &AppHandle,
    game_id: &str,
    payload_updated_at: Option<i64>,
) -> Option<Vec<u8>> {
    let payload_updated_at = payload_updated_at?.to_string();

    let paths = cache_paths(app, CacheKind::Autosplitter, game_id)?;
    let cache_path = paths.content;
    let stamp_path = paths.stamp;

    let cached_stamp = cache_path
        .exists()
        .then(|| std::fs::read_to_string(&stamp_path).ok())
        .flatten()
        .map(|s| s.trim().to_string());

    if cached_stamp.as_deref() == Some(payload_updated_at.as_str()) {
        return std::fs::read(&cache_path)
            .map_err(|e| mlog!(LogCat::Wasm, "[autosplitter] cache read error: {e}"))
            .ok();
    }

    let path = config::game_autosplitter_download_path(game_id);
    let mut req = ApiClient::new(app).authenticated()?.get(&path);
    if let Some(stamp) = cached_stamp.as_deref() {
        req = req.header(IF_NONE_MATCH, format!("\"{stamp}\""));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[autosplitter] fetch error: {e}"))
        .ok()?;

    let status = resp.status();
    if status == StatusCode::NOT_MODIFIED {
        let bytes = std::fs::read(&cache_path)
            .map_err(|e| mlog!(LogCat::Wasm, "[autosplitter] cache read error: {e}"))
            .ok()?;
        let _ = std::fs::write(&stamp_path, &payload_updated_at);
        return Some(bytes);
    }
    if !status.is_success() {
        if status != StatusCode::NOT_FOUND {
            mlog!(LogCat::Api, "[autosplitter] unexpected status: {status}");
        }
        return None;
    }

    let stamp = resp
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim_start_matches("W/").trim_matches('"').to_string())
        .unwrap_or(payload_updated_at);
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[autosplitter] read body error: {e}"))
        .ok()?
        .to_vec();

    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&cache_path, &bytes) {
        mlog!(LogCat::Wasm, "[autosplitter] cache write error: {e}");
    }
    let _ = std::fs::write(&stamp_path, stamp);

    Some(bytes)
}
