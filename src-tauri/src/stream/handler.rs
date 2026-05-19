/// v1 implementation: `WhipStreamHandle` — state tracked here, actual WebRTC
///   lives in the webview (browser API).
/// v2 swap: replace with `FfmpegStreamHandle` without touching `commands.rs`.
pub trait StreamHandle: Send + Sync {
    fn is_live(&self) -> bool;
    /// Signals that the stream should stop.
    /// For v1 (webview WHIP) this just flips the flag; the webview tears down
    /// the RTCPeerConnection independently when `notify_stream_stopped` fires.
    /// For v2 (ffmpeg) this will send SIGTERM to the child process.
    fn stop(&self);
}
