use reqwest::StatusCode;
use serde::Deserialize;
use tauri::AppHandle;

use crate::models::lobby::PlayerStatus;
use crate::models::LobbySetup;
use crate::{config, models::LobbyStatus};

use super::client::{ApiClient, ApiResponse};

/// Response from the lobby/current endpoint from backend (used only here)
#[derive(Debug, Deserialize)]
pub struct LobbyCurrentResponse {
    pub lobby_id: String,
    pub code: String,
    pub lobby_status: LobbyStatus,
    pub player_status: PlayerStatus,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
    pub race_start_at: Option<i64>,
}

pub async fn fetch_current_lobby(app: &AppHandle) -> Option<LobbySetup> {
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
        eprintln!(
            "[api] fetch_current_lobby unexpected status: {}",
            resp.status()
        );
        return None;
    }

    let body: ApiResponse<LobbyCurrentResponse> = resp
        .json()
        .await
        .map_err(|e| eprintln!("[api] fetch_current_lobby parse error: {e}"))
        .ok()?;

    let l: LobbyCurrentResponse = body.data;
    Some(LobbySetup {
        lobby_id: l.lobby_id,
        code: l.code,
        lobby_status: l.lobby_status,
        player_status: l.player_status,
        stream_key: l.stream_key,
        whip_url: l.whip_url,
        game_name: l.game_name,
        category_name: l.category_name,
        race_start_at: l.race_start_at,
    })
}
