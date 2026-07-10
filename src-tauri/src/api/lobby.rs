use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::counter::CounterSample;
use crate::models::lobby::PlayerStatus;
use crate::models::LobbySetup;
use crate::{config, models::LobbyStatus};

use super::client::{
    authed_get_json, authed_post_body_json_outcome, authed_post_body_void,
    authed_post_body_void_outcome, authed_post_returning, authed_post_void, PostOutcome,
};

// finish/forfeit
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerResult {
    pub player_status: PlayerStatus,
    pub finishing_time_ms: Option<u64>,
    pub finish_position: Option<u32>,
}

// lobby/current endpoint
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
    pub category_split_id: Option<String>,
    #[serde(default)]
    pub split_resource_updated_at: Option<String>,
    #[serde(default)]
    pub autosplitter_updated_at: Option<String>,
    #[serde(default)]
    pub counter_config_updated_at: Option<String>,
}

pub async fn fetch_current_lobby(app: &AppHandle) -> Option<LobbySetup> {
    let l: LobbyCurrentResponse =
        authed_get_json::<Option<LobbyCurrentResponse>>(app, config::LOBBY_CURRENT_PATH, "api")
            .await??;
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
        category_split_id: l.category_split_id,
        split_resource_updated_at: l.split_resource_updated_at,
        autosplitter_updated_at: l.autosplitter_updated_at,
        counter_config_updated_at: l.counter_config_updated_at,
    })
}

pub async fn post_stream_ready(app: &AppHandle, lobby_id: &str) -> Result<(), String> {
    authed_post_void(
        app,
        &config::lobby_stream_ready_path(lobby_id),
        "stream_ready",
    )
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
struct AutosplitStatusBody {
    connected: bool,
    splits_valid: bool,
    // true = the run already started
    run_in_progress: bool,
}

pub async fn post_autosplit_status(
    app: &AppHandle,
    lobby_id: &str,
    connected: bool,
    splits_valid: bool,
    run_in_progress: bool,
) -> Result<(), String> {
    authed_post_body_void(
        app,
        &config::lobby_autosplit_status_path(lobby_id),
        &AutosplitStatusBody {
            connected,
            splits_valid,
            run_in_progress,
        },
        "autosplit_status",
    )
    .await
    .ok_or_else(|| "[api] post_autosplit_status failed".to_string())
}

#[derive(Serialize)]
struct FinishPlayerBody {
    finishing_time_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_started_at_ms: Option<i64>,
}

pub async fn submit_finish(
    app: &AppHandle,
    lobby_id: &str,
    finishing_time_ms: u64,
    run_started_at_ms: Option<i64>,
) -> PostOutcome<PlayerResult> {
    authed_post_body_json_outcome(
        app,
        &config::lobby_finish_path(lobby_id),
        &FinishPlayerBody {
            finishing_time_ms,
            run_started_at_ms,
        },
        "finished",
    )
    .await
}

#[derive(Serialize)]
struct RunStartedBody {
    elapsed_ms: i64, // since when it is started
}

pub async fn submit_run_started(
    app: &AppHandle,
    lobby_id: &str,
    elapsed_ms: i64,
) -> PostOutcome<()> {
    authed_post_body_void_outcome(
        app,
        &config::lobby_run_started_path(lobby_id),
        &RunStartedBody { elapsed_ms },
        "run_started",
    )
    .await
}

pub async fn post_player_forfeited(
    app: &AppHandle,
    lobby_id: &str,
) -> Result<PlayerResult, String> {
    authed_post_returning(app, &config::lobby_forfeit_path(lobby_id), "forfeited")
        .await
        .ok_or_else(|| "[api] post_player_forfeited failed".to_string())
}

#[derive(Serialize)]
struct SubmitSplitBody<'a> {
    split_index: u32,
    segment_name: &'a str,
    start_ms: u64,
    end_ms: u64,
}

pub async fn submit_split(app: &AppHandle, split: &crate::state::PendingSplit) -> PostOutcome<()> {
    authed_post_body_void_outcome(
        app,
        &config::lobby_split_path(&split.lobby_id),
        &SubmitSplitBody {
            split_index: split.split_index,
            segment_name: &split.segment_name,
            start_ms: split.start_ms,
            end_ms: split.end_ms,
        },
        "split",
    )
    .await
}

#[derive(Serialize)]
struct SubmitCounterBody {
    counter_name: String,
    samples: Vec<CounterSample>,
}

pub async fn post_player_counter(
    app: &AppHandle,
    lobby_id: &str,
    counter_name: String,
    samples: Vec<CounterSample>,
) -> Result<(), String> {
    authed_post_body_void(
        app,
        &config::lobby_counter_path(lobby_id),
        &SubmitCounterBody {
            counter_name,
            samples,
        },
        "counter",
    )
    .await
    .ok_or_else(|| "[api] post_player_counter failed".to_string())
}
