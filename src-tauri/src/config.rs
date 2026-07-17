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

const DEEP_LINK_SCHEME: &str = match option_env!("DEEP_LINK_SCHEME") {
    Some(s) => s,
    None => "speedrace",
};

// OAuth client constants
pub const OAUTH_CLIENT_ID: &str = "tauri_desktop";

pub fn oauth_redirect_uri() -> &'static str {
    static CELL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CELL.get_or_init(|| format!("{DEEP_LINK_SCHEME}://auth/callback"))
}

pub const LOBBY_CURRENT_PATH: &str = "/api/v1/lobby/current";

pub fn lobby_stream_ready_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/stream-ready")
}

pub fn lobby_stream_stopped_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/stream-stopped")
}

pub fn lobby_autosplit_status_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/autosplit-status")
}

pub fn lobby_finish_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/finish")
}

pub fn lobby_run_started_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/run-started")
}

pub fn lobby_forfeit_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/forfeit")
}

pub fn lobby_vod_complete_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/vod-complete")
}

pub fn lobby_request_upload_ticket_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/request-upload-ticket")
}

pub fn split_resource_path(category_split_id: &str) -> String {
    format!("/api/v1/split-resources/{category_split_id}")
}

pub fn game_autosplitter_download_path(game_id: &str) -> String {
    format!("/api/v1/games/{game_id}/autosplitter/download")
}

pub fn game_counters_path(game_id: &str) -> String {
    format!("/api/v1/games/{game_id}/counters")
}

pub fn lobby_split_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/split")
}

pub fn lobby_counter_path(lobby_id: &str) -> String {
    format!("/api/v1/lobby/{lobby_id}/counter")
}

// WS reconnect - exponential back-off bounds
pub const WS_RECONNECT_BASE_SECS: u64 = 1;
pub const WS_RECONNECT_MAX_SECS: u64 = 30;
// Consecutive transient failures before giving up and showing the maintenance screen
pub const WS_MAX_RETRIES: u32 = 3;

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
