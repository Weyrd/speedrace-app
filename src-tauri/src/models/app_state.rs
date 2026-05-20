use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AppState {
    Unauthenticated,
    Connecting,
    Idle,
    StreamSetup,
    WaitingForStart,
    Racing,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WsStatus {
    Connected,
    Connecting,
    Disconnected,
}
