use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

const STORE_PATH: &str = "speedrace_auth.json";
const STORE_KEY: &str = "auth";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserData {
    pub id: String,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: String, // ISO 8601 / RFC 3339
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAuth {
    pub tokens: Tokens,
    pub user: UserData,
}

pub struct TokenStore {
    app: AppHandle,
}

impl TokenStore {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub fn save(&self, auth: &StoredAuth) -> Result<(), String> {
        let store = self.app.store(STORE_PATH).map_err(|e| e.to_string())?;
        store.set(
            STORE_KEY,
            serde_json::to_value(auth).map_err(|e| e.to_string())?,
        );
        store.save().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn load(&self) -> Option<StoredAuth> {
        let store = self.app.store(STORE_PATH).ok()?;
        let val = store.get(STORE_KEY)?;
        serde_json::from_value(val).ok()
    }

    pub fn clear(&self) -> Result<(), String> {
        let store = self.app.store(STORE_PATH).map_err(|e| e.to_string())?;
        store.delete(STORE_KEY);
        store.save().map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn update_tokens(&self, tokens: Tokens) -> Result<(), String> {
        let mut auth = self.load().ok_or("no stored auth to update")?;
        auth.tokens = tokens;
        self.save(&auth)
    }

    pub fn get_access_token(&self) -> Option<String> {
        self.load().map(|a| a.tokens.access_token)
    }

    #[allow(dead_code)]
    pub fn get_user(&self) -> Option<UserData> {
        self.load().map(|a| a.user)
    }

    pub fn time_until_expiry(&self) -> Duration {
        let auth = match self.load() {
            Some(a) => a,
            None => return Duration::ZERO,
        };
        seconds_until_expiry(&auth.tokens.expires_at)
    }

    pub fn is_expired(&self) -> bool {
        self.time_until_expiry() == Duration::ZERO
    }
}

pub fn seconds_until_expiry(expires_at: &str) -> Duration {
    let expires = match chrono::DateTime::parse_from_rfc3339(expires_at) {
        Ok(dt) => dt,
        Err(_) => return Duration::ZERO,
    };
    let diff = expires.signed_duration_since(Utc::now());
    if diff.num_seconds() <= 0 {
        Duration::ZERO
    } else {
        Duration::from_secs(diff.num_seconds() as u64)
    }
}
