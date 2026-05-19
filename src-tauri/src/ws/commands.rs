#[derive(Debug)]
pub enum WsCommand {
    StreamReady { lobby_id: String },
    StreamStopped { lobby_id: String },
    Disconnect,
}
