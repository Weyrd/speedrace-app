// ffmpeg streaming sidecar (Windows-only)

mod audio;
mod ffmpeg;
mod monitors;
mod pipeline;
pub mod preview;
mod thumbs;
mod types;
pub(crate) mod wgc;
mod window_list;

pub use monitors::*;
pub use thumbs::*;
pub use types::*;
pub use window_list::*;

use crate::events::STREAM_STATUS;
use crate::logging::{mlog, LogCat};
use crate::models::lobby::RaceType;
use crate::models::AppState;
use crate::state::SharedState;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};
use tokio::sync::watch;

const PUBLISH_LIVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

pub fn emit_status(app: &AppHandle, state: StreamState, message: Option<String>) {
    let _ = app.emit(STREAM_STATUS, StreamStatusPayload { state, message });
}

pub fn current_source(app: &AppHandle, state: &SharedState) -> CaptureSource {
    let session = state.lock().ok().and_then(|g| g.capture_source.clone());
    session.unwrap_or(CaptureSource::Monitor {
        index: crate::settings::load_stream_settings(app).monitor_index,
    })
}

pub async fn start(
    app: &AppHandle,
    state: &SharedState,
    live_tx: Option<tokio::sync::oneshot::Sender<()>>,
) -> Result<Option<PathBuf>, String> {
    let (whip_url, session_source, race_type, game_name) = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        if guard.app_state != AppState::StreamSetup {
            return Err("stream can only start from StreamSetup".into());
        }
        if guard.stream.is_some() {
            return Err("stream already running".into());
        }
        let lobby = guard.lobby.as_ref().ok_or("no active lobby")?;
        if lobby.whip_url.is_empty() {
            return Err("lobby has no whip_url".into());
        }
        (
            lobby.whip_url.clone(),
            guard.capture_source.clone(),
            lobby.race_type,
            lobby.game_name.clone(),
        )
    };
    let settings = load_settings(app, session_source);

    let ffmpeg_path = ffmpeg::resolve_ffmpeg_path()?;
    let replay_base = resolve_replay_base(app, race_type, &game_name);
    let replay_out = replay_base.clone();

    let (stop_tx, stop_rx) = watch::channel(false);
    let app_c = app.clone();
    let state_c = state.clone();
    let whip = whip_url.clone();

    //  supervisor audio + ffmpeg
    let join = tauri::async_runtime::spawn(async move {
        ffmpeg::supervise(
            app_c,
            state_c,
            LaunchSpec {
                ffmpeg_path,
                whip_url: whip,
                settings,
                replay_base,
            },
            stop_rx,
            live_tx,
        )
        .await;
    });

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.stream = Some(StreamSession { stop_tx, join });
    }

    emit_status(app, StreamState::Connecting, None);
    Ok(replay_out)
}

pub async fn publish(app: &AppHandle, state: &SharedState, lobby_id: &str) -> Result<(), String> {
    preview::stop(state).await;

    let (live_tx, live_rx) = tokio::sync::oneshot::channel::<()>();
    let replay = match start(app, state, Some(live_tx)).await {
        Ok(r) => r,
        Err(e) => {
            let _ = preview::start(app, state).await;
            return Err(e);
        }
    };

    let live = tokio::time::timeout(PUBLISH_LIVE_TIMEOUT, live_rx).await;
    if !matches!(live, Ok(Ok(()))) {
        return publish_fail(app, state, replay, "stream did not go live").await;
    }

    if let Err(e) = crate::api::lobby::post_stream_ready(app, lobby_id).await {
        return publish_fail(app, state, replay, &format!("stream-ready failed: {e}")).await;
    }

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.app_state = AppState::WaitingForStart;
    }
    Ok(())
}

async fn publish_fail(
    app: &AppHandle,
    state: &SharedState,
    replay: Option<PathBuf>,
    msg: &str,
) -> Result<(), String> {
    shutdown(app, state, true).await;
    if let Some(p) = replay {
        let _ = std::fs::remove_file(&p);
    }
    let _ = preview::start(app, state).await;
    mlog!(LogCat::Stream, "[publish] failed: {msg}");
    Err(msg.to_string())
}

// Graceful stop; also the single choke point that kills any local preview
pub async fn shutdown(app: &AppHandle, state: &SharedState, graceful: bool) {
    preview::stop(state).await;
    let session = {
        match state.lock() {
            Ok(mut g) => g.stream.take(),
            Err(_) => return,
        }
    };
    let Some(session) = session else { return };

    let _ = session.stop_tx.send(true);

    if graceful {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(4), session.join).await;
    } else {
        session.join.abort();
    }
    mlog!(
        LogCat::Stream,
        "[stream] shutdown complete (graceful={graceful})"
    );
    let _ = app;
}

pub fn shutdown_spawn(app: &AppHandle, state: &SharedState) {
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        shutdown(&app, &state, true).await;
    });
}

fn load_settings(app: &AppHandle, session_source: Option<CaptureSource>) -> StreamSettings {
    let s = crate::settings::load_stream_settings(app);
    StreamSettings {
        source: session_source.unwrap_or(CaptureSource::Monitor {
            index: s.monitor_index,
        }),
        bitrate_kbps: s.bitrate_kbps,
        framerate: s.framerate,
    }
}

// Ranked race have a VOD automatic
fn resolve_replay_base(app: &AppHandle, race_type: RaceType, game_name: &str) -> Option<PathBuf> {
    let settings = crate::settings::load_stream_settings(app);
    if race_type != RaceType::Ranked && !settings.replay_casual {
        return None;
    }
    let dir = PathBuf::from(settings.replay_dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        mlog!(
            LogCat::Stream,
            "[replay] cannot create dir {}: {e}",
            dir.display()
        );
        return None;
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let game: String = game_name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    Some(dir.join(format!("speedrace_{game}_{stamp}.mp4")))
}

// Mid-race restarts cant append a ended MP4
pub(crate) fn segment_path(base: &std::path::Path, attempt: u32) -> PathBuf {
    if attempt == 0 {
        return base.to_path_buf();
    }
    let stem = base.file_stem().map(|s| s.to_string_lossy().into_owned());
    let ext = base.extension().map(|s| s.to_string_lossy().into_owned());
    match (base.parent(), stem, ext) {
        (Some(dir), Some(stem), Some(ext)) => dir.join(format!("{stem}_pt{}.{ext}", attempt + 1)),
        _ => base.to_path_buf(),
    }
}

pub fn sweep_old_replays(app: &AppHandle) {
    let s = crate::settings::load_stream_settings(app);
    if !s.replay_autodelete {
        return;
    }
    let cutoff = std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(
        crate::settings::REPLAY_RETENTION_DAYS * 86_400,
    ));
    let Some(cutoff) = cutoff else { return };
    let Ok(entries) = std::fs::read_dir(&s.replay_dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("mp4") {
            continue;
        }
        let expired = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|mt| mt < cutoff)
            .unwrap_or(false);
        if expired {
            match std::fs::remove_file(&p) {
                Ok(()) => mlog!(LogCat::Stream, "[replay] auto-deleted {}", p.display()),
                Err(err) => mlog!(
                    LogCat::Stream,
                    "[replay] delete failed {}: {err}",
                    p.display()
                ),
            }
        }
    }
}
