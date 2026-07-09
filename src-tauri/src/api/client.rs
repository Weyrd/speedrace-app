use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use reqwest::StatusCode;
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

    let body: ApiResponse<T> = resp
        .json()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] parse error: {e}"))
        .ok()?;
    Some(body.data)
}

// Same as authed_get_json but raw body bytes
#[allow(dead_code)]
pub async fn authed_get_bytes(app: &AppHandle, path: &str, log_tag: &str) -> Option<Vec<u8>> {
    let client = ApiClient::new(app);
    let authed = client.authenticated()?;

    let resp = authed
        .get(path)
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

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] read body error: {e}"))
        .ok()?
        .to_vec();
    Some(bytes)
}

/// POST with no body, checks success only.
pub async fn authed_post_void(app: &AppHandle, path: &str, log_tag: &str) -> Option<()> {
    let client = ApiClient::new(app);
    let authed = client.authenticated()?;
    let resp = authed
        .post(path)
        .send()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] post error: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        mlog!(
            LogCat::Api,
            "[{log_tag}] unexpected status: {}",
            resp.status()
        );
        return None;
    }
    Some(())
}

/// POST with no body, parses JSON response envelope.
pub async fn authed_post_returning<R: DeserializeOwned>(
    app: &AppHandle,
    path: &str,
    log_tag: &str,
) -> Option<R> {
    let client = ApiClient::new(app);
    let authed = client.authenticated()?;
    let resp = authed
        .post(path)
        .send()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] post error: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        mlog!(
            LogCat::Api,
            "[{log_tag}] unexpected status: {}",
            resp.status()
        );
        return None;
    }
    let body: ApiResponse<R> = resp
        .json()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] parse error: {e}"))
        .ok()?;
    Some(body.data)
}

/// POST with JSON body, checks success only.
pub async fn authed_post_body_void<B: Serialize>(
    app: &AppHandle,
    path: &str,
    body: &B,
    log_tag: &str,
) -> Option<()> {
    let client = ApiClient::new(app);
    let authed = client.authenticated()?;
    let resp = authed
        .post(path)
        .json(body)
        .send()
        .await
        .map_err(|e| mlog!(LogCat::Api, "[{log_tag}] post error: {e}"))
        .ok()?;
    if !resp.status().is_success() {
        mlog!(
            LogCat::Api,
            "[{log_tag}] unexpected status: {}",
            resp.status()
        );
        return None;
    }
    Some(())
}

pub enum PostOutcome<R> {
    Ok(R),
    Rejected,  // 4xx: won't change on retry (already done / gone / bad request)
    Transient, // network error, 5xx, or no token yet: worth retrying
}

pub async fn authed_post_body_void_outcome<B: Serialize>(
    app: &AppHandle,
    path: &str,
    body: &B,
    log_tag: &str,
) -> PostOutcome<()> {
    let client = ApiClient::new(app);
    let Some(authed) = client.authenticated() else {
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
        PostOutcome::Ok(())
    } else if status.is_server_error() {
        mlog!(LogCat::Api, "[{log_tag}] server error: {status}");
        PostOutcome::Transient
    } else {
        mlog!(LogCat::Api, "[{log_tag}] rejected: {status}");
        PostOutcome::Rejected
    }
}

pub async fn authed_post_body_json_outcome<B: Serialize, R: DeserializeOwned>(
    app: &AppHandle,
    path: &str,
    body: &B,
    log_tag: &str,
) -> PostOutcome<R> {
    let client = ApiClient::new(app);
    let Some(authed) = client.authenticated() else {
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
        match resp.json::<ApiResponse<R>>().await {
            Ok(b) => PostOutcome::Ok(b.data),
            Err(e) => {
                mlog!(LogCat::Api, "[{log_tag}] parse error: {e}");
                PostOutcome::Transient
            }
        }
    } else if status.is_server_error() {
        mlog!(LogCat::Api, "[{log_tag}] server error: {status}");
        PostOutcome::Transient
    } else {
        mlog!(LogCat::Api, "[{log_tag}] rejected: {status}");
        PostOutcome::Rejected
    }
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
