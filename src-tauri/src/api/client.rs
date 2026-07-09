use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::{Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::auth::token_store::TokenStore;
use crate::config;
use crate::logging::{mlog, LogCat};
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

pub enum PostOutcome<R> {
    Ok(R),
    Rejected,  // 4xx: won't change on retry (already done / gone / bad request)
    Transient, // network error, 5xx, or no token yet: worth retrying
}

impl<R> PostOutcome<R> {
    fn map<U>(self, f: impl FnOnce(R) -> U) -> PostOutcome<U> {
        match self {
            PostOutcome::Ok(v) => PostOutcome::Ok(f(v)),
            PostOutcome::Rejected => PostOutcome::Rejected,
            PostOutcome::Transient => PostOutcome::Transient,
        }
    }
}

// Sends a request; returns None on 404, non-2xx, or network error.
async fn send_check(req: reqwest::RequestBuilder, log_tag: &str) -> Option<Response> {
    let resp = req
        .send()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] fetch error: {e}"))
        .ok()?;
    if resp.status() == StatusCode::NOT_FOUND {
        return None;
    }
    if !resp.status().is_success() {
        mlog!(
            LogCat::Api,
            "[{log_tag}] unexpected status: {}",
            resp.status()
        );
        return None;
    }
    Some(resp)
}

// Sends a POST with body; maps 2xx→Ok(resp), 5xx/network→Transient, 4xx→Rejected.
async fn send_outcome<B: Serialize>(
    app: &AppHandle,
    path: &str,
    body: &B,
    log_tag: &str,
) -> PostOutcome<Response> {
    let Some(authed) = ApiClient::new(app).authenticated() else {
        return PostOutcome::Transient; // no token yet; a refresh may restore it
    };
    let resp = match authed.post(path).json(body).send().await {
        Ok(r) => r,
        Err(e) => {
            mlog!(LogCat::Api, "[{log_tag}] post error: {e}");
            return PostOutcome::Transient;
        }
    };
    let status = resp.status();
    if status.is_success() {
        PostOutcome::Ok(resp)
    } else if status.is_server_error() {
        mlog!(LogCat::Api, "[{log_tag}] server error: {status}");
        PostOutcome::Transient
    } else {
        mlog!(LogCat::Api, "[{log_tag}] rejected: {status}");
        PostOutcome::Rejected
    }
}

async fn parse_json<T: DeserializeOwned>(resp: Response, log_tag: &str) -> Option<T> {
    resp.json::<ApiResponse<T>>()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] parse error: {e}"))
        .ok()
        .map(|b| b.data)
}

// --- Public API ---

pub async fn authed_get_json<T: DeserializeOwned>(
    app: &AppHandle,
    path: &str,
    log_tag: &str,
) -> Option<T> {
    let resp = send_check(ApiClient::new(app).authenticated()?.get(path), log_tag).await?;
    parse_json(resp, log_tag).await
}

#[allow(dead_code)]
pub async fn authed_get_bytes(app: &AppHandle, path: &str, log_tag: &str) -> Option<Vec<u8>> {
    let resp = send_check(ApiClient::new(app).authenticated()?.get(path), log_tag).await?;
    resp.bytes()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] read body error: {e}"))
        .ok()
        .map(|b| b.to_vec())
}

pub async fn authed_post_void(app: &AppHandle, path: &str, log_tag: &str) -> Option<()> {
    send_check(ApiClient::new(app).authenticated()?.post(path), log_tag)
        .await
        .map(|_| ())
}

pub async fn authed_post_returning<R: DeserializeOwned>(
    app: &AppHandle,
    path: &str,
    log_tag: &str,
) -> Option<R> {
    let resp = send_check(ApiClient::new(app).authenticated()?.post(path), log_tag).await?;
    parse_json(resp, log_tag).await
}

pub async fn authed_post_body_void<B: Serialize>(
    app: &AppHandle,
    path: &str,
    body: &B,
    log_tag: &str,
) -> Option<()> {
    send_check(
        ApiClient::new(app).authenticated()?.post(path).json(body),
        log_tag,
    )
    .await
    .map(|_| ())
}

pub async fn authed_post_body_void_outcome<B: Serialize>(
    app: &AppHandle,
    path: &str,
    body: &B,
    log_tag: &str,
) -> PostOutcome<()> {
    send_outcome(app, path, body, log_tag).await.map(|_| ())
}

pub async fn authed_post_body_json_outcome<B: Serialize, R: DeserializeOwned>(
    app: &AppHandle,
    path: &str,
    body: &B,
    log_tag: &str,
) -> PostOutcome<R> {
    match send_outcome(app, path, body, log_tag).await {
        PostOutcome::Ok(resp) => match parse_json(resp, log_tag).await {
            Some(data) => PostOutcome::Ok(data),
            None => PostOutcome::Transient,
        },
        PostOutcome::Rejected => PostOutcome::Rejected,
        PostOutcome::Transient => PostOutcome::Transient,
    }
}

// --- Client structs ---

pub struct ApiClient {
    http: reqwest::Client,
    app: AppHandle,
}

impl ApiClient {
    pub fn new(app: &AppHandle) -> Self {
        Self {
            http: reqwest::Client::new(),
            app: app.clone(),
        }
    }

    pub fn authenticated(&self) -> Option<AuthenticatedClient> {
        let token = TokenStore::new(self.app.clone()).get_access_token()?;
        Some(AuthenticatedClient {
            http: self.http.clone(),
            token,
        })
    }

    pub fn base_url(path: &str) -> String {
        config::api_url(path)
    }
}

pub struct AuthenticatedClient {
    http: reqwest::Client,
    token: String,
}

impl AuthenticatedClient {
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.request(reqwest::Method::GET, path)
    }

    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.request(reqwest::Method::POST, path)
    }

    #[allow(dead_code)]
    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.request(reqwest::Method::DELETE, path)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, ApiClient::base_url(path))
            .headers(self.auth_headers())
    }

    fn auth_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", self.token)) {
            headers.insert(AUTHORIZATION, val);
        }
        headers
    }
}
