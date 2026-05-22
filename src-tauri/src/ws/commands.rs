use serde::Serialize;

pub const MSG_TYPE_STREAM_READY: &str = "stream_ready";
pub const MSG_TYPE_STREAM_STOPPED: &str = "stream_stopped";
pub const MSG_TYPE_PLAYER_FINISHED: &str = "player_finished";
pub const MSG_TYPE_PLAYER_FORFEITED: &str = "player_forfeited";

#[derive(Debug)]
pub enum WsCommand {
    StreamReady { lobby_id: String },
    StreamStopped { lobby_id: String },
    PlayerFinished { lobby_id: String, finishing_time_ms: u64 },
    PlayerForfeited { lobby_id: String },
    Disconnect,
}

/// Typed outgoing message envelope (serialized to JSON for sending over WS)
#[derive(Debug, Serialize)]
struct OutgoingMessage<'a> {
    r#type: &'a str,
    lobby_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    finishing_time_ms: Option<u64>,
}

impl WsCommand {
    /// Serialize to JSON for sending over the WebSocket. Returns None for Disconnect.
    pub fn to_json(&self) -> Option<String> {
        let msg = match self {
            WsCommand::StreamReady { lobby_id } => OutgoingMessage {
                r#type: MSG_TYPE_STREAM_READY,
                lobby_id,
                finishing_time_ms: None,
            },
            WsCommand::StreamStopped { lobby_id } => OutgoingMessage {
                r#type: MSG_TYPE_STREAM_STOPPED,
                lobby_id,
                finishing_time_ms: None,
            },
            WsCommand::PlayerFinished { lobby_id, finishing_time_ms } => OutgoingMessage {
                r#type: MSG_TYPE_PLAYER_FINISHED,
                lobby_id,
                finishing_time_ms: Some(*finishing_time_ms),
            },
            WsCommand::PlayerForfeited { lobby_id } => OutgoingMessage {
                r#type: MSG_TYPE_PLAYER_FORFEITED,
                lobby_id,
                finishing_time_ms: None,
            },
            WsCommand::Disconnect => return None,
        };
        Some(serde_json::to_string(&msg).expect("WsCommand serialization failed"))
    }
}
