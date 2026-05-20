use crate::models::RaceResults;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    LobbySetup(LobbySetupMsg),
    Countdown(CountdownMsg),
    RaceResults(RaceResultsMsg),
    LobbyClosed(LobbyClosedMsg),
    Ping,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LobbyClosedMsg {
    pub lobby_id: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LobbySetupMsg {
    pub lobby_id: String,
    pub stream_key: String,
    pub whip_url: String,
    pub game_name: String,
    pub category_name: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct CountdownMsg {
    pub race_start_at: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RaceResultsMsg {
    pub results: RaceResults,
}
