use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum StreamState {
    Connecting,
    Live,
    Reconnecting,
    Error,
    Stopped,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamStatusPayload {
    pub state: StreamState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum CaptureSource {
    Monitor { index: u32 },
    Window { hwnd: u64, title: String },
}

#[derive(Debug, Clone)]
pub struct StreamSettings {
    pub source: CaptureSource,
    pub bitrate_kbps: u32,
    pub framerate: u32,
    pub resolution: u32,
}

pub struct LaunchSpec {
    pub ffmpeg_path: PathBuf,
    pub whip_url: String,
    pub settings: StreamSettings,
    pub replay_base: Option<PathBuf>,
    pub encoder: Encoder,
}

pub(crate) enum Outcome {
    Stopped, // external graceful stop requested
    Died,    // ffmpeg exited or stalled unexpectedly
}

pub struct StreamSession {
    pub(crate) stop_tx: watch::Sender<bool>,
    pub(crate) join: tauri::async_runtime::JoinHandle<()>,
}

pub struct PreviewSession {
    pub(crate) id: u64,
    pub(crate) stop_tx: watch::Sender<bool>,
    pub(crate) join: tauri::async_runtime::JoinHandle<()>,
}

// "stream:preview" payload
#[derive(Serialize, Clone)]
#[serde(untagged)]
pub(crate) enum PreviewEvent {
    Frame { frame: String },
    Error { error: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct WindowInfo {
    pub hwnd: u64,
    pub title: String,
    pub process_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoder {
    X264,
    Nvenc,
    Amf,
}

impl Encoder {
    pub const ALL: [Encoder; 3] = [Encoder::X264, Encoder::Nvenc, Encoder::Amf];

    fn names(self) -> (&'static str, &'static str) {
        match self {
            Encoder::X264 => ("libx264", "x264"),
            Encoder::Nvenc => ("h264_nvenc", "nvenc"),
            Encoder::Amf => ("h264_amf", "amf"),
        }
    }

    pub fn name(self) -> &'static str {
        self.names().0
    }

    // None is "auto".
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Self::ALL.into_iter().find(|e| {
            let (a, b) = e.names();
            a == s || b == s
        })
    }
}

pub enum AudioSource {
    #[cfg(windows)]
    Pipe(String),
    Silent, // fallback when WASAPI loopback is unavailable
}

pub(crate) type StopFlag = Arc<AtomicBool>;

// WGC window capture -> fixed-size BGRA letterbox
pub struct WgcHandle {
    pub pipe_name: String,
    pub width: u32,
    pub height: u32,
    #[cfg(windows)]
    pub(crate) control: Option<windows_capture::capture::CaptureControl<WgcCapture, WgcError>>,
    #[cfg(windows)]
    pub(crate) writer: Option<tauri::async_runtime::JoinHandle<()>>,
    #[cfg(windows)]
    pub(crate) stop: StopFlag,
}

#[cfg(windows)]
pub(crate) type WgcError = Box<dyn std::error::Error + Send + Sync>;

#[cfg(windows)]
pub(crate) struct WgcFlags {
    pub(crate) target_w: u32,
    pub(crate) target_h: u32,
    pub(crate) latest: Arc<std::sync::Mutex<Vec<u8>>>,
    pub(crate) closed: StopFlag,
}

#[cfg(windows)]
pub(crate) struct WgcCapture {
    pub(crate) target_w: u32,
    pub(crate) target_h: u32,
    pub(crate) latest: Arc<std::sync::Mutex<Vec<u8>>>,
    pub(crate) closed: StopFlag,
    pub(crate) last_dims: (u32, u32),
}
