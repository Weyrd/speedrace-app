use crate::api::client::ApiResponse;
use crate::auth::token_store::{StoredAuth, TokenStore, Tokens, UserData};
use crate::config;
use crate::events::AUTH_STATE;
use crate::logging::{mlog, LogCat};
use crate::models::{AuthStatePayload, AuthUser, LoginError};
use crate::state::SharedState;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;
use url::Url;

static PENDING_PKCE_VERIFIER: Mutex<Option<String>> = Mutex::new(None);

pub fn emit_auth_state(app: &AppHandle, payload: AuthStatePayload) {
    mlog!(LogCat::Auth, "[auth] emitting auth:state → {:?}", payload);
    let result = app.emit(AUTH_STATE, payload);
    mlog!(LogCat::Auth, "[auth] emit result: {:?}", result);
}

pub fn open_browser_login(app: &AppHandle) -> Result<(), LoginError> {
    {
        let pending = PENDING_PKCE_VERIFIER
            .lock()
            .map_err(|e| LoginError::System(e.to_string()))?;
        if pending.is_some() {
            mlog!(
                LogCat::Auth,
                "[auth] login already in progress, cancelling and starting new login"
            );
            drop(pending);
            clear_pending_verifier();
        }
    }

    let pkce = generate_pkce();

    {
        let mut pending = PENDING_PKCE_VERIFIER
            .lock()
            .map_err(|e| LoginError::System(e.to_string()))?;
        *pending = Some(pkce.code_verifier);
    }

    let url = build_auth_url(&pkce.code_challenge).map_err(LoginError::System)?;

    mlog!(LogCat::Auth, "[auth] opening browser login: {url}");

    app.opener()
        .open_url(&url, None::<String>)
        .map_err(|e| LoginError::System(e.to_string()))
}

pub async fn handle_callback(app: AppHandle, url: String, shared_state: SharedState) {
    mlog!(
        LogCat::Auth,
        "[auth] handle_callback called with url: {url}"
    );

    let params = parse_query_params(&url);

    if let Some(error) = params.get("error") {
        mlog!(LogCat::Auth, "[auth] OAuth error in callback: {error}");
        clear_pending_verifier();
        emit_auth_state(&app, AuthStatePayload::Unauthenticated);
        return;
    }

    let auth_code = match params.get("code") {
        Some(c) => c.clone(),
        None => {
            mlog!(LogCat::Auth, "[auth] deep link received but no code: {url}");
            clear_pending_verifier();
            emit_auth_state(&app, AuthStatePayload::Unauthenticated);
            return;
        }
    };

    mlog!(
        LogCat::Auth,
        "[auth] got auth code, consuming PKCE verifier..."
    );

    let code_verifier = {
        let mut pending = match PENDING_PKCE_VERIFIER.lock() {
            Ok(g) => g,
            Err(e) => {
                mlog!(LogCat::Auth, "[auth] PKCE lock poisoned: {e}");
                return;
            }
        };
        match pending.take() {
            Some(v) => v,
            None => {
                mlog!(
                    LogCat::Auth,
                    "[auth] no pending PKCE verifier - possible replay attack, ignoring"
                );
                return;
            }
        }
    };

    mlog!(LogCat::Auth, "[auth] exchanging code for tokens...");

    match exchange_code(auth_code, code_verifier).await {
        Ok(stored) => {
            mlog!(
                LogCat::Auth,
                "[auth] token exchange OK, user: {}",
                stored.user.username
            );

            let store = TokenStore::new(app.clone());
            if let Err(e) = store.save(&stored) {
                mlog!(LogCat::Auth, "[auth] token store save error: {e}");
                emit_auth_state(&app, AuthStatePayload::Unauthenticated);
                return;
            }

            mlog!(
                LogCat::Auth,
                "[auth] tokens saved, updating shared state..."
            );

            {
                let mut guard = shared_state.lock().unwrap();
                guard.user = Some(stored.user.clone());
                guard.app_state = crate::models::AppState::Connecting;
            }

            mlog!(
                LogCat::Auth,
                "[auth] emitting authenticated state for user: {}",
                stored.user.username
            );

            emit_auth_state(
                &app,
                AuthStatePayload::Authenticated {
                    user: AuthUser {
                        username: stored.user.username.clone(),
                    },
                },
            );

            crate::lifecycle::start_background_loops(&app, &shared_state);

            {
                let mut guard = shared_state.lock().unwrap();
                if guard.ws_status == crate::models::WsStatus::Connected
                    && guard.app_state == crate::models::AppState::Connecting
                {
                    guard.app_state = crate::models::AppState::Idle;
                    drop(guard);
                    let _ = app.emit(crate::events::APP_STATE, crate::models::AppState::Idle);
                }
            }
        }

        Err(e) => {
            mlog!(LogCat::Auth, "[auth] token exchange failed: {e}");
            emit_auth_state(&app, AuthStatePayload::Unauthenticated);
        }
    }
}

// Helper
fn clear_pending_verifier() {
    if let Ok(mut pending) = PENDING_PKCE_VERIFIER.lock() {
        *pending = None;
    }
}

fn build_auth_url(code_challenge: &str) -> Result<String, String> {
    let base = format!("{}{}", config::BACKEND_URL, config::AUTH_DESKTOP_PATH);
    let mut url = Url::parse(&base).map_err(|e| format!("invalid auth base URL: {e}"))?;

    url.query_pairs_mut()
        .append_pair("client_id", config::OAUTH_CLIENT_ID)
        .append_pair("redirect_uri", config::oauth_redirect_uri())
        .append_pair("response_type", "code")
        .append_pair("code_challenge", code_challenge)
        .append_pair("code_challenge_method", "S256");

    Ok(url.into())
}

fn generate_pkce() -> PkceChallenge {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(bytes);
    let digest = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = URL_SAFE_NO_PAD.encode(digest);
    PkceChallenge {
        code_verifier,
        code_challenge,
    }
}

async fn exchange_code(auth_code: String, code_verifier: String) -> Result<StoredAuth, String> {
    let client = reqwest::Client::new();
    let resp = client
        .post(config::api_url(config::AUTH_TOKEN_PATH))
        .json(&TokenExchangeRequest {
            auth_code: &auth_code,
            client_id: config::OAUTH_CLIENT_ID,
            redirect_uri: config::oauth_redirect_uri(),
            grant_type: config::GRANT_TYPE_AUTH_CODE,
            code_verifier: &code_verifier,
        })
        .send()
        .await
        .map_err(|e| format!("exchange_code network error: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("exchange_code failed ({status}): {body}"));
    }

    let body: ApiResponse<TokenResponse> = resp
        .json()
        .await
        .map_err(|e| format!("exchange_code parse error: {e}"))?;

    let token = body.data;
    Ok(StoredAuth {
        tokens: Tokens {
            access_token: token.access_token,
            refresh_token: token.refresh_token,
            expires_at: token.expires_at,
        },
        user: UserData {
            id: token.user.id,
            username: token.user.username,
        },
    })
}

fn parse_query_params(url: &str) -> std::collections::HashMap<String, String> {
    url::Url::parse(url)
        .map(|u| {
            u.query_pairs()
                .map(|(k, v)| (k.into_owned(), v.into_owned()))
                .collect()
        })
        .unwrap_or_default()
}

struct PkceChallenge {
    code_verifier: String,
    code_challenge: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    expires_at: String,
    user: TokenResponseUser,
}

#[derive(Debug, Deserialize)]
struct TokenResponseUser {
    id: String,
    username: String,
}

#[derive(Debug, Serialize)]
struct TokenExchangeRequest<'a> {
    auth_code: &'a str,
    client_id: &'a str,
    redirect_uri: &'a str,
    grant_type: &'a str,
    code_verifier: &'a str,
}
