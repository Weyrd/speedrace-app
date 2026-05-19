use reqwest::StatusCode;
use tauri::AppHandle;

use crate::auth::token_store::TokenStore;
use crate::config;
use crate::state::{ApiResponse, LobbyInfo};


pub async fn fetch_current_lobby(app: &AppHandle) -> Option<LobbyInfo> {
    let token = TokenStore::new(app.clone()).get_access_token()?;

    let client = reqwest::Client::new();
    let resp = client
        .get(config::api_url(config::LOBBY_CURRENT_PATH))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .map_err(|e| eprintln!("[lobby] fetch_current_lobby network error: {e}"))
        .ok()?;

    if resp.status() == StatusCode::NOT_FOUND {
        return None; // not in a lobby 
    }

    if !resp.status().is_success() {
        eprintln!("[lobby] fetch_current_lobby unexpected status: {}", resp.status());
        return None;
    }

    let body: ApiResponse<LobbyInfo> = resp
        .json()
        .await
        .map_err(|e| eprintln!("[lobby] fetch_current_lobby parse error: {e}"))
        .ok()?;

    let l = body.data;
    Some(LobbyInfo {
        lobby_id: l.lobby_id,
        stream_key: l.stream_key,
        whip_url: l.whip_url,
        game_name: l.game_name,
        category_name: l.category_name,
    })
}