use serde::{Deserialize, Serialize};
//TODO: delete ce fichier entier?
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlayerStatus {
    Preparing,
    Ready,
    RaceInProgress,
    Finished,
    Forfeited,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerResult {
    pub user_id: String,
    pub username: String,
    pub player_status: PlayerStatus,
    pub finishing_time_ms: Option<u64>,
    pub finish_position: Option<u32>,
}
