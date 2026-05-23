use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum AuthStatePayload {
    Authenticated { user: AuthUser },
    Unauthenticated,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub username: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", content = "message")]
pub enum LoginError {
    System(String),
}
