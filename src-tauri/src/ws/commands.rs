pub const MSG_TYPE_STREAM_READY: &str = "stream_ready";
pub const MSG_TYPE_STREAM_STOPPED: &str = "stream_stopped";

#[derive(Debug)]
pub enum WsCommand {
    StreamReady { lobby_id: String },
    StreamStopped { lobby_id: String },
    Disconnect,
}
