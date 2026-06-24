use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::models::lobby::PlayerStatus;
use crate::models::LobbySetup;
use crate::{config, models::LobbyStatus};

use super::client::{
    authed_get_json, authed_post_body_json, authed_post_body_void, authed_post_returning,
    authed_post_void,
};

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
    #[serde(default)]
    pub game_id: String,
    #[serde(default)]
    pub category_id: String,
    #[serde(default)]
    pub split_resource_updated_at: Option<String>,
    #[serde(default)]
    pub autosplitter_updated_at: Option<String>,
}

pub async fn fetch_current_lobby(app: &AppHandle) -> Option<LobbySetup> {
    let l: LobbyCurrentResponse =
        authed_get_json(app, config::LOBBY_CURRENT_PATH, "api").await?;
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
        game_id: l.game_id,
        category_id: l.category_id,
        split_resource_updated_at: l.split_resource_updated_at,
        autosplitter_updated_at: l.autosplitter_updated_at,
    })
}

// Tauri -> Backend HTTP actions

pub async fn post_stream_ready(app: &AppHandle, lobby_id: &str) -> Result<(), String> {
    authed_post_void(app, &config::lobby_stream_ready_path(lobby_id), "stream_ready")
        .await
        .ok_or_else(|| "[api] post_stream_ready failed".to_string())
}

pub async fn post_stream_stopped(app: &AppHandle, lobby_id: &str) -> Result<(), String> {
    authed_post_void(
        app,
        &config::lobby_stream_stopped_path(lobby_id),
        "stream_stopped",
    )
    .await
    .ok_or_else(|| "[api] post_stream_stopped failed".to_string())
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
    authed_post_body_json(
        app,
        &config::lobby_finish_path(lobby_id),
        &FinishPlayerBody { finishing_time_ms },
        "finished",
    )
    .await
    .ok_or_else(|| "[api] post_player_finished failed".to_string())
}

pub async fn post_player_forfeited(
    app: &AppHandle,
    lobby_id: &str,
) -> Result<PlayerResult, String> {
    authed_post_returning(app, &config::lobby_forfeit_path(lobby_id), "forfeited")
        .await
        .ok_or_else(|| "[api] post_player_forfeited failed".to_string())
}

#[allow(dead_code)]
#[derive(Serialize)]
struct SubmitSplitBody {
    split_index: u32,
    segment_name: String,
    start_ms: u64,
    end_ms: u64,
}

#[allow(dead_code)]
pub async fn post_player_split(
    app: &AppHandle,
    lobby_id: &str,
    split_index: u32,
    segment_name: String,
    start_ms: u64,
    end_ms: u64,
) -> Result<(), String> {
    authed_post_body_void(
        app,
        &config::lobby_split_path(lobby_id),
        &SubmitSplitBody {
            split_index,
            segment_name,
            start_ms,
            end_ms,
        },
        "split",
    )
    .await
    .ok_or_else(|| "[api] post_player_split failed".to_string())
}

#[allow(dead_code)]
#[derive(Serialize)]
struct SubmitCounterBody {
    counter_name: String,
    value: i64,
    split_index: Option<u32>,
    timestamp_ms: u64,
}

#[allow(dead_code)]
pub async fn post_player_counter(
    app: &AppHandle,
    lobby_id: &str,
    counter_name: String,
    value: i64,
    split_index: Option<u32>,
    timestamp_ms: u64,
) -> Result<(), String> {
    authed_post_body_void(
        app,
        &config::lobby_counter_path(lobby_id),
        &SubmitCounterBody {
            counter_name,
            value,
            split_index,
            timestamp_ms,
        },
        "counter",
    )
    .await
    .ok_or_else(|| "[api] post_player_counter failed".to_string())
}
