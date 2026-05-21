use reqwest::StatusCode;
use serde::Deserialize;
use tauri::AppHandle;

use crate::config;
use crate::models::LobbySetup;

use super::client::{ApiClient, ApiResponse};

/// Response from the lobby/current endpoint, combining lobby data + race state.
pub struct LobbyCurrentResponse {
    pub lobby: LobbySetup,
    pub status: Option<String>,
    pub race_start_at: Option<i64>,
}

/// Raw API response shape — private, only used for deserialization.
#[derive(Debug, Deserialize)]
struct LobbyApiData {
    pub lobby_id: String,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
    pub status: Option<String>,
    pub race_start_at: Option<i64>,
}

pub async fn fetch_current_lobby(app: &AppHandle) -> Option<LobbyCurrentResponse> {
    let client = ApiClient::new(app);
    let authed = client.authenticated()?;

    let resp = authed
        .get(config::LOBBY_CURRENT_PATH)
        .send()
        .await
        .map_err(|e| eprintln!("[api] fetch_current_lobby network error: {e}"))
        .ok()?;

    if resp.status() == StatusCode::NOT_FOUND {
        return None;
    }

    if !resp.status().is_success() {
        eprintln!("[api] fetch_current_lobby unexpected status: {}", resp.status());
        return None;
    }

    let body: ApiResponse<LobbyApiData> = resp
        .json()
        .await
        .map_err(|e| eprintln!("[api] fetch_current_lobby parse error: {e}"))
        .ok()?;

    let l = body.data;
    Some(LobbyCurrentResponse {
        lobby: LobbySetup {
            lobby_id: l.lobby_id,
            stream_key: l.stream_key,
            whip_url: l.whip_url,
            game_name: l.game_name,
            category_name: l.category_name,
        },
        status: l.status,
        race_start_at: l.race_start_at,
    })
}
