use serde::Deserialize;
use tauri::AppHandle;

use crate::config;

use super::cache::{cache_paths, CacheKind};
use super::client::authed_get_json;

#[derive(Deserialize)]
struct CategorySplitLssResponse {
    lss_content: String,
    updated_at: String,
}

#[allow(dead_code)]
pub async fn fetch_category_split_lss(
    app: &AppHandle,
    category_id: &str,
    payload_updated_at: Option<&str>,
) -> Option<String> {
    let payload_updated_at = payload_updated_at?;

    let paths = cache_paths(app, CacheKind::Split, category_id)?;
    let cache_path = paths.content;
    let stamp_path = paths.stamp;

    if cache_path.exists() {
        if let Ok(cached_stamp) = std::fs::read_to_string(&stamp_path) {
            if cached_stamp.trim() == payload_updated_at {
                return std::fs::read_to_string(&cache_path)
                    .map_err(|e| eprintln!("[split] cache read error: {e}"))
                    .ok();
            }
        }
    }

    let path = config::category_split_resource_path(category_id);
    let body: CategorySplitLssResponse = authed_get_json(app, &path, "split").await?;

    let content = body.lss_content;
    let stamp = body.updated_at;

    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&cache_path, &content) {
        eprintln!("[split] cache write error: {e}");
    }
    let _ = std::fs::write(&stamp_path, &stamp);

    Some(content)
}
