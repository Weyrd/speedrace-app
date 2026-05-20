use std::time::Duration;
use tauri::AppHandle;

use crate::api::client::ApiResponse;
use crate::auth::oauth::emit_auth_state;
use crate::auth::token_store::{seconds_until_expiry, TokenStore, Tokens};
use crate::config;
use crate::models::AuthStatePayload;
use crate::state::SharedState;


pub async fn token_refresh_loop(app: AppHandle, shared_state: SharedState) {
    let store = TokenStore::new(app.clone());

    loop {
        let auth = match store.load() {
            Some(a) => a,
            None => {
                eprintln!("[refresh] no stored auth, stopping refresh loop");
                break;
            }
        };

        let expires_in = seconds_until_expiry(&auth.tokens.expires_at);

        let sleep = expires_in
            .saturating_sub(Duration::from_secs(config::TOKEN_REFRESH_MARGIN_SECS));
        tokio::time::sleep(sleep).await;

        match refresh_access_token(&auth.tokens.refresh_token).await {
            Ok(new_tokens) => {
                if let Err(e) = store.update_tokens(new_tokens) {
                    eprintln!("[refresh] failed to persist new tokens: {e}");
                }
            }
            Err(e) => {
                eprintln!("[refresh] token expired or revoked: {e}");
                logout_and_notify(&app, &store);
                break;
            }
        }
    }

    // Reset guard on exit
    if let Ok(mut guard) = shared_state.lock() {
        guard.refresh_loop_running = false;
    }
}


pub async fn do_refresh(refresh_token: &str) -> Result<Tokens, String> {
    refresh_access_token(refresh_token).await
}


fn logout_and_notify(app: &AppHandle, store: &TokenStore) {
    let _ = store.clear();
    emit_auth_state(app, AuthStatePayload::Unauthenticated);
}

/// POST /api/v1/auth/desktop/refresh
async fn refresh_access_token(refresh_token: &str) -> Result<Tokens, String> {
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
        .map_err(|e| format!("refresh network error: {e}"))?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err("refresh token expired or revoked (401)".into());
    }

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("refresh failed ({status}): {body}"));
    }

    #[derive(serde::Deserialize)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: String,
        expires_at: String,
    }

    let parsed: ApiResponse<RefreshResponse> = resp
        .json()
        .await
        .map_err(|e| format!("refresh parse error: {e}"))?;

    Ok(Tokens {
        access_token: parsed.data.access_token,
        refresh_token: parsed.data.refresh_token,
        expires_at: parsed.data.expires_at,
    })
}