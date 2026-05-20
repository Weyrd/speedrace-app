use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;

use crate::auth::token_store::TokenStore;
use crate::config;
use tauri::AppHandle;

#[derive(Debug, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
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

    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.http
            .post(ApiClient::base_url(path))
            .headers(self.auth_headers())
    }

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
