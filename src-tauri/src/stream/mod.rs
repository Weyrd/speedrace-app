mod audio;
pub(crate) mod capture;
#[cfg(windows)]
pub(crate) mod capture_pipe;
pub mod encoder;
mod ffmpeg;
pub(crate) mod gamecapture;
mod monitors;
mod pipeline;
pub mod preview;
pub(crate) mod replay;
mod thumbs;
mod types;
pub(crate) mod wgc;
mod window_list;

pub(crate) use ffmpeg::{ffmpeg_command, NULL_SINK};
pub(crate) use pipeline::replay_encoder_args;

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

// try to preselect windows based on wasm process attached
#[cfg(windows)]
pub(crate) async fn auto_select_game_window(app: &AppHandle, state: &SharedState, pid: u32) {
    let current = {
        let Ok(g) = state.lock() else { return };
        if g.stream.is_some() {
            return;
        }
        g.capture_source.clone()
    };
    let Some((hwnd, title)) = window_list::game_window_for_pid(pid) else {
        return;
    };
    if let Some(CaptureSource::Window { hwnd: cur, .. }) = current {
        if cur == hwnd {
            return;
        }
    }
    let source = CaptureSource::Window { hwnd, title };
    {
        let Ok(mut g) = state.lock() else { return };
        if g.stream.is_some() {
            return;
        }
        g.capture_source = Some(source.clone());
    }
    mlog!(
        LogCat::Stream,
        "[source] auto-selected game window {hwnd:#x} for pid {pid}"
    );
    let _ = app.emit(crate::events::STREAM_SOURCE, &source);
    if let Err(e) = preview::restart(app, state).await {
        mlog!(LogCat::Stream, "[source] preview restart: {e}");
    }
}

pub async fn start(
    app: &AppHandle,
    state: &SharedState,
    live_tx: Option<tokio::sync::oneshot::Sender<()>>,
) -> Result<Option<PathBuf>, String> {
    let (whip_url, session_source, race_type, game_name, category_name, username) = {
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
            lobby.category_name.clone(),
            guard.user.as_ref().map(|u| u.username.clone()),
        )
    };
    let settings = load_settings(app, session_source);

    let ffmpeg_path = ffmpeg::resolve_ffmpeg_path()?;
    let replay_base = resolve_replay_base(
        app,
        race_type,
        &game_name,
        &category_name,
        username.as_deref(),
    );
    let replay_out = replay_base.clone();

    let pref = Encoder::parse(&crate::settings::load_stream_settings(app).encoder);
    let encoder = encoder::select(pref, replay_base.is_some()).await;
    mlog!(LogCat::Stream, "[stream] encoder: {}", encoder.name());

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
                encoder,
            },
            stop_rx,
            live_tx,
        )
        .await;
    });

    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.stream = Some(StreamSession { stop_tx, join });
        guard.replay_base = replay_out.clone();
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
        if let Some(a) = replay::ReplayArtifacts::open(&p) {
            a.discard();
        }
        if let Ok(mut g) = state.lock() {
            g.replay_base = None;
            g.countdown_start_at_ms = None;
        }
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
            Ok(mut g) => {
                let s = g.stream.take();
                if s.is_some() {
                    g.stream_finalizing = true;
                }
                s
            }
            Err(_) => return,
        }
    };
    let Some(session) = session else {
        await_finalized(state).await;
        return;
    };

    let _ = session.stop_tx.send(true);

    if graceful {
        let _ = tokio::time::timeout(std::time::Duration::from_secs(4), session.join).await;
    } else {
        session.join.abort();
    }
    if let Ok(mut g) = state.lock() {
        g.stream_finalizing = false;
    }
    mlog!(
        LogCat::Stream,
        "[stream] shutdown complete (graceful={graceful})"
    );
    let _ = app;
}

async fn await_finalized(state: &SharedState) {
    for _ in 0..300 {
        match state.lock() {
            Ok(g) if g.stream_finalizing => {}
            _ => return,
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

pub fn shutdown_spawn(app: &AppHandle, state: &SharedState) {
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        shutdown(&app, &state, true).await;
    });
}

pub(crate) fn ffmpeg_path() -> Result<PathBuf, String> {
    ffmpeg::resolve_ffmpeg_path()
}

fn load_settings(app: &AppHandle, session_source: Option<CaptureSource>) -> StreamSettings {
    let s = crate::settings::load_stream_settings(app);
    StreamSettings {
        source: session_source.unwrap_or(CaptureSource::Monitor {
            index: s.monitor_index,
        }),
        bitrate_kbps: s.bitrate_kbps,
        framerate: s.framerate,
        resolution: s.resolution,
    }
}

fn sanitize(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\'')
        .map(|c| {
            if r#"<>:"/\|?*"#.contains(c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn title_category(category_name: &[String]) -> Option<String> {
    match category_name {
        [] => None,
        [only] => Some(only.clone()),
        [.., parent, leaf] => Some(format!("{parent} ({leaf})")),
    }
}

fn resolve_replay_base(
    app: &AppHandle,
    race_type: RaceType,
    game_name: &str,
    category_name: &[String],
    username: Option<&str>,
) -> Option<PathBuf> {
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
    let stamp = chrono::Local::now().format("%d-%m-%Y %Hh%Mm%Ss");

    let mut parts = vec![sanitize(game_name)];
    parts.extend(title_category(category_name).map(|c| sanitize(&c)));
    parts.extend(username.map(sanitize));
    parts.retain(|p| !p.is_empty());
    parts.push(stamp.to_string());

    Some(dir.join(format!("speedrace_{}.mp4", parts.join(" - "))))
}

pub(crate) const SEGMENT_SECS: u32 = 5;

#[derive(Clone)]
pub(crate) struct ReplayRun {
    pub dir: PathBuf,
    pub pattern: PathBuf,
    pub list: PathBuf,
}

pub(crate) fn replay_run(base: &std::path::Path, run: u32) -> Option<ReplayRun> {
    let dir = replay::parts_dir(base)?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        mlog!(
            LogCat::Stream,
            "[replay] cannot create {}: {e}",
            dir.display()
        );
        return None;
    }
    Some(ReplayRun {
        pattern: dir.join(replay::segment_pattern(run)),
        list: dir.join(replay::list_name(run)),
        dir,
    })
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
        let is_parts_dir = replay::is_parts_dir(&p);
        if !is_parts_dir && p.extension().and_then(|x| x.to_str()) != Some("mp4") {
            continue;
        }
        let expired = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|mt| mt < cutoff)
            .unwrap_or(false);
        if expired {
            let removed = if is_parts_dir {
                std::fs::remove_dir_all(&p)
            } else {
                std::fs::remove_file(&p)
            };
            match removed {
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
