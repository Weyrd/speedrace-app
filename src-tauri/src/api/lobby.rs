use reqwest::StatusCode;
use serde::Deserialize;
use tauri::AppHandle;

use crate::config;
use crate::models::race::PlayerStatus;
use crate::models::LobbySetup;

use super::client::{ApiClient, ApiResponse};

/// Response from the lobby/current endpoint, combining lobby data + race state.
pub struct LobbyCurrentResponse {
    pub lobby: LobbySetup,
    pub player_status: PlayerStatus,
}

/// Matches the backend's LobbyCurrentDto — private, only used for deserialization.
#[derive(Debug, Deserialize)]
struct LobbyCurrentDto {
    pub lobby_id: String,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
    pub player_status: PlayerStatus,
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

    let body: ApiResponse<LobbyCurrentDto> = resp
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
            race_start_at: l.race_start_at,
        },
        player_status: l.player_status,
    })
}
