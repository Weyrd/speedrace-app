use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::counter::CounterSample;
use crate::logging::{mlog, LogCat};
use crate::models::lobby::{PlayerStatus, RaceConfig, RaceType};
use crate::models::LobbySetup;
use crate::{config, models::LobbyStatus};

use super::client::{
    authed_get_json, authed_post_body_json_outcome, authed_post_body_void,
    authed_post_body_void_outcome, authed_post_json_outcome, authed_post_returning,
    authed_post_void, authed_post_void_outcome, ApiError, ApiErrorCode, PostOutcome,
};

const STREAM_READY_ATTEMPTS: u32 = 8;
const STREAM_READY_RETRY: std::time::Duration = std::time::Duration::from_secs(1);
const VOD_COMPLETE_ATTEMPTS: u32 = 4;
const UPLOAD_TICKET_ATTEMPTS: u32 = 3;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayerResult {
    pub player_status: PlayerStatus,
    pub finishing_time_ms: Option<u64>,
    pub finish_position: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LobbyCurrentHandicap {
    #[serde(default)]
    pub start_delay_ms: i64,
}

#[derive(Debug, Deserialize)]
pub struct LobbyCurrentResponse {
    pub lobby_id: String,
    pub code: String,
    pub lobby_status: LobbyStatus,
    #[serde(default)]
    pub race_type: RaceType,
    pub player_status: PlayerStatus,
    pub stream_key: String,
    pub whip_url: String,
    #[serde(default)]
    pub whep_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
    pub max_duration_minutes: u32,
    pub race_start_at_ms: Option<i64>,
    #[serde(default)]
    pub handicap: Option<LobbyCurrentHandicap>,
    pub expires_at_ms: i64,
    #[serde(default)]
    pub game_id: String,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub race_config: Option<RaceConfig>,
}

pub async fn fetch_current_lobby(app: &AppHandle) -> Option<LobbySetup> {
    let l: LobbyCurrentResponse =
        authed_get_json::<Option<LobbyCurrentResponse>>(app, config::LOBBY_CURRENT_PATH, "api")
            .await??;
    Some(LobbySetup {
        lobby_id: l.lobby_id,
        code: l.code,
        lobby_status: l.lobby_status,
        race_type: l.race_type,
        player_status: l.player_status,
        stream_key: l.stream_key,
        whip_url: l.whip_url,
        whep_url: l.whep_url,
        game_name: l.game_name,
        category_name: l.category_name,
        max_duration_minutes: l.max_duration_minutes,
        race_start_at_ms: l
            .race_start_at_ms
            .map(|s| s + l.handicap.unwrap_or_default().start_delay_ms),
        expires_at_ms: l.expires_at_ms,
        game_id: l.game_id,
        category_id: l.category_id,
        race_config: l.race_config,
    })
}

pub async fn post_stream_ready(app: &AppHandle, lobby_id: &str) -> Result<(), String> {
    let path = config::lobby_stream_ready_path(lobby_id);
    for attempt in 1..=STREAM_READY_ATTEMPTS {
        match authed_post_void_outcome(app, &path, "stream_ready").await {
            PostOutcome::Ok(()) => return Ok(()),
            PostOutcome::Rejected(ApiErrorCode::StreamNotReady) | PostOutcome::Transient
                if attempt < STREAM_READY_ATTEMPTS =>
            {
                mlog!(
                    LogCat::Api,
                    "[stream_ready] not confirmed yet (attempt {attempt}), retrying"
                );
                tokio::time::sleep(STREAM_READY_RETRY).await;
            }
            PostOutcome::Rejected(ApiErrorCode::PlayerNotInLobby) => {
                crate::lifecycle::spawn_resync_if_still_in(app, lobby_id);
                return Err("[api] post_stream_ready: no longer in this lobby".to_string());
            }
            PostOutcome::Rejected(code) => {
                return Err(format!("[api] post_stream_ready refused: {code:?}"));
            }
            PostOutcome::Transient => break,
        }
    }
    Err("[api] post_stream_ready failed".to_string())
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
    run_in_progress: bool,
}

pub async fn post_autosplit_status(
    app: &AppHandle,
    lobby_id: &str,
    connected: bool,
    splits_valid: bool,
    run_in_progress: bool,
) -> Result<(), String> {
    match authed_post_body_void_outcome(
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
    {
        PostOutcome::Ok(()) => Ok(()),
        PostOutcome::Rejected(ApiErrorCode::PlayerNotInLobby) => {
            crate::lifecycle::spawn_resync_if_still_in(app, lobby_id);
            Err("[api] post_autosplit_status: no longer in this lobby".to_string())
        }
        PostOutcome::Rejected(code) => {
            Err(format!("[api] post_autosplit_status refused: {code:?}"))
        }
        PostOutcome::Transient => Err("[api] post_autosplit_status failed".to_string()),
    }
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
    elapsed_ms: i64,
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
struct VodCompleteBody<'a> {
    upload_ticket: &'a str,
    video_id: &'a str,
}

pub async fn post_vod_complete(
    app: &AppHandle,
    lobby_id: &str,
    upload_ticket: &str,
    video_id: &str,
) -> Result<(), String> {
    let path = config::lobby_vod_complete_path(lobby_id);
    let body = VodCompleteBody {
        upload_ticket,
        video_id,
    };
    for attempt in 1..=VOD_COMPLETE_ATTEMPTS {
        match authed_post_body_void_outcome(app, &path, &body, "vod_complete").await {
            PostOutcome::Ok(()) => return Ok(()),
            PostOutcome::Rejected(
                ApiErrorCode::UploadVideoNotFound | ApiErrorCode::RaceHistoryNotFound,
            )
            | PostOutcome::Transient
                if attempt < VOD_COMPLETE_ATTEMPTS =>
            {
                tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
            }
            PostOutcome::Rejected(code) => {
                return Err(format!("[api] post_vod_complete refused: {code:?}"));
            }
            PostOutcome::Transient => {}
        }
    }
    Err("[api] post_vod_complete failed".to_string())
}

#[derive(Debug, Deserialize)]
pub struct UploadTicketResponse {
    pub upload_ticket: String,
    pub resumable_url: String,
}

pub async fn post_request_upload_ticket(
    app: &AppHandle,
    lobby_id: &str,
    fresh: bool,
) -> Result<UploadTicketResponse, String> {
    let mut path = config::lobby_request_upload_ticket_path(lobby_id);
    if fresh {
        path.push_str("?fresh=true");
    }
    for attempt in 1..=UPLOAD_TICKET_ATTEMPTS {
        match authed_post_json_outcome(app, &path, "request_upload_ticket").await {
            PostOutcome::Ok(ticket) => return Ok(ticket),
            PostOutcome::Rejected(code) => {
                return Err(format!(
                    "[api] post_request_upload_ticket refused: {code:?}"
                ));
            }
            PostOutcome::Transient if attempt < UPLOAD_TICKET_ATTEMPTS => {
                tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
            }
            PostOutcome::Transient => {}
        }
    }
    Err("[api] post_request_upload_ticket failed".to_string())
}

pub enum ResumeTicket {
    Ready(UploadTicketResponse),
    NotOwed,
    RetryLater,
}

fn not_owed(code: &ApiErrorCode) -> bool {
    matches!(
        code,
        ApiErrorCode::UploadNotEligible
            | ApiErrorCode::RaceHistoryNotFound
            | ApiErrorCode::LobbyNotFound
            | ApiErrorCode::PlayerNotInLobby
    )
}

pub async fn request_upload_ticket_for_resume(app: &AppHandle, lobby_id: &str) -> ResumeTicket {
    let Some(authed) = crate::api::client::ApiClient::new(app).authenticated() else {
        return ResumeTicket::RetryLater;
    };
    let resp = match authed
        .post(&config::lobby_request_upload_ticket_path(lobby_id))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            mlog!(LogCat::Api, "[resume_upload] fetch error: {e}");
            return ResumeTicket::RetryLater;
        }
    };

    if resp.status().is_success() {
        return match resp
            .json::<crate::api::client::ApiResponse<UploadTicketResponse>>()
            .await
        {
            Ok(b) => ResumeTicket::Ready(b.data),
            Err(e) => {
                mlog!(LogCat::Api, "[resume_upload] parse error: {e}");
                ResumeTicket::RetryLater
            }
        };
    }

    let status = resp.status();
    let not_owed = resp
        .json::<ApiError>()
        .await
        .is_ok_and(|e| not_owed(&e.error_code));
    mlog!(
        LogCat::Api,
        "[resume_upload] rejected: {status} not_owed={not_owed}"
    );
    if not_owed {
        ResumeTicket::NotOwed
    } else {
        ResumeTicket::RetryLater
    }
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
