pub const BACKEND_URL: &str = match option_env!("BACKEND_URL") {
    Some(url) => url,
    None => "http://localhost:8080",
};

pub const WS_PATH: &str = "/ws/app";

// OAuth desktop routes
pub const AUTH_DESKTOP_PATH: &str = "/auth/desktop";
pub const AUTH_TOKEN_PATH: &str = "/api/v1/auth/token";
pub const AUTH_REFRESH_PATH: &str = "/api/v1/auth/desktop/refresh";
pub const AUTH_LOGOUT_PATH: &str = "/api/v1/auth/desktop/logout";

// OAuth client constants
pub const AUTH_CALLBACK_PREFIX: &str = "momentum://auth/callback";
pub const OAUTH_CLIENT_ID: &str = "tauri_desktop";
pub const OAUTH_REDIRECT_URI: &str = "momentum://auth/callback";

pub const LOBBY_CURRENT_PATH: &str = "/api/v1/lobby/current";

pub fn lobby_stream_ready_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/stream-ready")
}

pub fn lobby_stream_stopped_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/stream-stopped")
}

pub fn lobby_finish_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/finish")
}

pub fn lobby_forfeit_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/forfeit")
}

// WS reconnect - exponential back-off bounds
pub const WS_RECONNECT_BASE_SECS: u64 = 1;
pub const WS_RECONNECT_MAX_SECS: u64 = 30;

// OAuth grant type strings
pub const GRANT_TYPE_AUTH_CODE: &str = "authorization_code";
pub const GRANT_TYPE_REFRESH: &str = "refresh_token";

pub const TOKEN_REFRESH_MARGIN_SECS: u64 = 60; // refresh 60s before expiry

pub fn ws_url() -> String {
    let host = BACKEND_URL
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let scheme = if BACKEND_URL.starts_with("https://") {
        "wss"
    } else {
        "ws"
    };
    format!("{scheme}://{host}{WS_PATH}")
}

pub fn api_url(path: &str) -> String {
    format!("{BACKEND_URL}{path}")
}
