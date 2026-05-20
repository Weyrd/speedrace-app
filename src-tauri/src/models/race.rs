use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerResult {
    pub user_id: String,
    pub username: String,
    pub finishing_time_ms: Option<u64>,
    pub forfeited: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaceResults {
    pub players: Vec<PlayerResult>,
}
