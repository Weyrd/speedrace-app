use std::fmt;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::api::client::ApiResponse;
use crate::auth::oauth::emit_auth_state;
use crate::auth::token_store::{seconds_until_expiry, TokenStore, Tokens};
use crate::config;
use crate::events::APP_STATE;
use crate::logging::{mlog, LogCat};
use crate::models::{AppState, AuthStatePayload};
use crate::state::{LockGlobalState, SharedState};

const TRANSIENT_RETRY_BASE_SECS: u64 = 5;
const TRANSIENT_RETRY_MAX_SECS: u64 = 60;

#[derive(Debug)]
pub enum RefreshError {
    Revoked(String),
    Banned,
    Transient(String),
}

impl fmt::Display for RefreshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefreshError::Revoked(why) => write!(f, "session ended: {why}"),
            RefreshError::Banned => write!(f, "account banned"),
            RefreshError::Transient(why) => write!(f, "refresh unavailable: {why}"),
        }
    }
}

struct Rotation {
    from: String,
    to: Tokens,
}

static LAST_ROTATION: tokio::sync::Mutex<Option<Rotation>> = tokio::sync::Mutex::const_new(None);

pub async fn token_refresh_loop(app: AppHandle, shared_state: SharedState) {
    let store = TokenStore::new(app.clone());
    let mut transient_backoff = Duration::from_secs(TRANSIENT_RETRY_BASE_SECS);

    loop {
        let Some(auth) = store.load() else {
            mlog!(
                LogCat::Auth,
                "[refresh] no stored auth, stopping refresh loop"
            );
            break;
        };

        let expires_in = seconds_until_expiry(auth.tokens.expires_at_ms);
        let sleep =
            expires_in.saturating_sub(Duration::from_secs(config::TOKEN_REFRESH_MARGIN_SECS));
        tokio::time::sleep(sleep).await;

        let Some(auth) = store.load() else {
            break;
        };
        if seconds_until_expiry(auth.tokens.expires_at_ms)
            > Duration::from_secs(config::TOKEN_REFRESH_MARGIN_SECS)
        {
            continue;
        }

        match do_refresh(&auth.tokens.refresh_token).await {
            Ok(new_tokens) => {
                transient_backoff = Duration::from_secs(TRANSIENT_RETRY_BASE_SECS);
                if let Err(e) = store.update_tokens(new_tokens) {
                    mlog!(LogCat::Auth, "[refresh] failed to persist new tokens: {e}");
                }
            }
            Err(RefreshError::Transient(e)) => {
                mlog!(
                    LogCat::Auth,
                    "[refresh] {e}, retrying in {transient_backoff:?}"
                );
                tokio::time::sleep(transient_backoff).await;
                transient_backoff =
                    (transient_backoff * 2).min(Duration::from_secs(TRANSIENT_RETRY_MAX_SECS));
            }
            Err(RefreshError::Banned) => {
                mlog!(LogCat::Auth, "[refresh] account banned");
                end_session_banned(&app, &store);
                break;
            }
            Err(e @ RefreshError::Revoked(_)) => {
                mlog!(LogCat::Auth, "[refresh] {e}");
                logout_and_notify(&app, &store);
                break;
            }
        }
    }

    shared_state.lock_state().refresh_loop_running = false;
}

pub async fn do_refresh(refresh_token: &str) -> Result<Tokens, RefreshError> {
    let mut last = LAST_ROTATION.lock().await;
    if let Some(rotation) = last.as_ref().filter(|r| r.from == refresh_token) {
        mlog!(
            LogCat::Auth,
            "[refresh] token already rotated by another caller, reusing the result"
        );
        return Ok(rotation.to.clone());
    }
    let tokens = refresh_access_token(refresh_token).await?;
    *last = Some(Rotation {
        from: refresh_token.to_string(),
        to: tokens.clone(),
    });
    Ok(tokens)
}

pub fn end_session_banned(app: &AppHandle, store: &TokenStore) {
    use tauri::Manager;
    let _ = store.clear();
    let state = app.state::<SharedState>();
    crate::stream::shutdown_spawn(app, &state);
    {
        let mut guard = state.lock_state();
        guard.app_state = AppState::Banned;
        guard.lobby = None;
        guard.race_start_at = None;
        guard.pending_finish = None;
    }
    let _ = app.emit(APP_STATE, AppState::Banned);
}

fn logout_and_notify(app: &AppHandle, store: &TokenStore) {
    use tauri::Manager;
    let _ = store.clear();
    let state = app.state::<crate::state::SharedState>();
    crate::stream::shutdown_spawn(app, &state);
    emit_auth_state(app, AuthStatePayload::Unauthenticated);
}

#[derive(serde::Deserialize)]
struct RefreshErrorBody {
    error_code: String,
}

fn classify_failure(status: reqwest::StatusCode, error_code: Option<&str>) -> RefreshError {
    if status == reqwest::StatusCode::FORBIDDEN && error_code == Some("ACCOUNT_BANNED") {
        return RefreshError::Banned;
    }
    if status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return RefreshError::Transient(format!("status {status}"));
    }
    RefreshError::Revoked(format!(
        "status {status} {}",
        error_code.unwrap_or("without error code")
    ))
}

async fn refresh_access_token(refresh_token: &str) -> Result<Tokens, RefreshError> {
    #[derive(serde::Serialize)]
    struct RefreshRequest<'a> {
        refresh_token: &'a str,
        grant_type: &'a str,
    }

    let body = RefreshRequest {
        refresh_token,
        grant_type: crate::config::GRANT_TYPE_REFRESH,
    };

    let client = reqwest::Client::new();
    let resp = client
        .post(crate::config::api_url(config::AUTH_REFRESH_PATH))
        .json(&body)
        .send()
        .await
        .map_err(|e| RefreshError::Transient(format!("network error: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let code = resp
            .json::<RefreshErrorBody>()
            .await
            .ok()
            .map(|b| b.error_code);
        return Err(classify_failure(status, code.as_deref()));
    }

    #[derive(serde::Deserialize)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: String,
        expires_at_ms: i64,
    }

    let parsed: ApiResponse<RefreshResponse> = resp
        .json()
        .await
        .map_err(|e| RefreshError::Revoked(format!("unreadable refresh response: {e}")))?;

    Ok(Tokens {
        access_token: parsed.data.access_token,
        refresh_token: parsed.data.refresh_token,
        expires_at_ms: parsed.data.expires_at_ms,
    })
}
