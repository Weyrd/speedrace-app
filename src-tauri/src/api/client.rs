use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::auth::token_store::TokenStore;
use crate::config;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

pub async fn authed_get_json<T: DeserializeOwned>(
    app: &AppHandle,
    path: &str,
    log_tag: &str,
) -> Option<T> {
    let client = ApiClient::new(app);
    let authed = client.authenticated()?;

    let resp = authed
        .get(path)
        .send()
        .await
        .map_err(|e| eprintln!("[{log_tag}] fetch error: {e}"))
        .ok()?;

    if resp.status() == StatusCode::NOT_FOUND {
        return None;
    }
    if !resp.status().is_success() {
        eprintln!("[{log_tag}] unexpected status: {}", resp.status());
        return None;
    }

    let body: ApiResponse<T> = resp
        .json()
        .await
        .map_err(|e| eprintln!("[{log_tag}] parse error: {e}"))
        .ok()?;
    Some(body.data)
}

// Same flow as authed_get_json but returns the raw body bytes (no JSON envelope).
#[allow(dead_code)]
pub async fn authed_get_bytes(app: &AppHandle, path: &str, log_tag: &str) -> Option<Vec<u8>> {
    let client = ApiClient::new(app);
    let authed = client.authenticated()?;

    let resp = authed
        .get(path)
        .send()
        .await
        .map_err(|e| eprintln!("[{log_tag}] fetch error: {e}"))
        .ok()?;

    if resp.status() == StatusCode::NOT_FOUND {
        return None;
    }
    if !resp.status().is_success() {
        eprintln!("[{log_tag}] unexpected status: {}", resp.status());
        return None;
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| eprintln!("[{log_tag}] read body error: {e}"))
        .ok()?
        .to_vec();
    Some(bytes)
}

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

    /// Returns None if no stored access token is available.
    pub fn authenticated(&self) -> Option<AuthenticatedClient<'_>> {
        let token = TokenStore::new(self.app.clone()).get_access_token()?;
        Some(AuthenticatedClient {
            http: &self.http,
            token,
        })
    }

    pub fn base_url(path: &str) -> String {
        config::api_url(path)
    }
}

pub struct AuthenticatedClient<'a> {
    http: &'a reqwest::Client,
    token: String,
}

impl<'a> AuthenticatedClient<'a> {
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.http
            .get(ApiClient::base_url(path))
            .headers(self.auth_headers())
    }
    #[allow(dead_code)]
    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.http
            .post(ApiClient::base_url(path))
            .headers(self.auth_headers())
    }
    #[allow(dead_code)]
    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.http
            .delete(ApiClient::base_url(path))
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
