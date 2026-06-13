use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::models::lobby::PlayerStatus;
use crate::models::LobbySetup;
use crate::{config, models::LobbyStatus};

use super::client::{ApiClient, ApiResponse};

/// Result returned by the finish/forfeit HTTP endpoints.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerResult {
    pub player_status: PlayerStatus,
    pub finishing_time_ms: Option<u64>,
    pub finish_position: Option<u32>,
}

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
    pub max_duration_minutes: u32,
    pub race_start_at: Option<i64>,
    pub expires_at: i64,
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
        max_duration_minutes: l.max_duration_minutes,
        race_start_at: l.race_start_at,
        expires_at: l.expires_at,
    })
}

// Tauri -> Backend HTTP actions

pub async fn post_stream_ready(app: &AppHandle, lobby_id: &str) -> Result<(), String> {
    let client = ApiClient::new(app);
    let authed = client
        .authenticated()
        .ok_or_else(|| "Not authenticated".to_string())?;

    let resp = authed
        .post(&config::lobby_stream_ready_path(lobby_id))
        .send()
        .await
        .map_err(|e| format!("[api] post_stream_ready network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "[api] post_stream_ready server error: {}",
            resp.status()
        ));
    }
    Ok(())
}

pub async fn post_stream_stopped(app: &AppHandle, lobby_id: &str) -> Result<(), String> {
    let client = ApiClient::new(app);
    let authed = client
        .authenticated()
        .ok_or_else(|| "Not authenticated".to_string())?;

    let resp = authed
        .post(&config::lobby_stream_stopped_path(lobby_id))
        .send()
        .await
        .map_err(|e| format!("[api] post_stream_stopped network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "[api] post_stream_stopped server error: {}",
            resp.status()
        ));
    }
    Ok(())
}

#[derive(Serialize)]
struct FinishPlayerBody {
    finishing_time_ms: u64,
}

pub async fn post_player_finished(
    app: &AppHandle,
    lobby_id: &str,
    finishing_time_ms: u64,
) -> Result<PlayerResult, String> {
    let client = ApiClient::new(app);
    let authed = client
        .authenticated()
        .ok_or_else(|| "Not authenticated".to_string())?;

    let resp = authed
        .post(&config::lobby_finish_path(lobby_id))
        .json(&FinishPlayerBody { finishing_time_ms })
        .send()
        .await
        .map_err(|e| format!("[api] post_player_finished network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "[api] post_player_finished server error: {}",
            resp.status()
        ));
    }

    let body: ApiResponse<PlayerResult> = resp
        .json()
        .await
        .map_err(|e| format!("[api] post_player_finished parse error: {e}"))?;
    Ok(body.data)
}

pub async fn post_player_forfeited(
    app: &AppHandle,
    lobby_id: &str,
) -> Result<PlayerResult, String> {
    let client = ApiClient::new(app);
    let authed = client
        .authenticated()
        .ok_or_else(|| "Not authenticated".to_string())?;

    let resp = authed
        .post(&config::lobby_forfeit_path(lobby_id))
        .send()
        .await
        .map_err(|e| format!("[api] post_player_forfeited network error: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "[api] post_player_forfeited server error: {}",
            resp.status()
        ));
    }

    let body: ApiResponse<PlayerResult> = resp
        .json()
        .await
        .map_err(|e| format!("[api] post_player_forfeited parse error: {e}"))?;
    Ok(body.data)
}
