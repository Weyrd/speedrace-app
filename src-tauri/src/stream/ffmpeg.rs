use super::pipeline;
use super::{
    audio, emit_status, Encoder, EncoderStatusPayload, LaunchSpec, Outcome, ReplayRun, StreamState,
};
#[cfg(windows)]
use super::PreviewEvent;
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::{LockGlobalState, SharedState};
#[cfg(windows)]
use base64::Engine;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::{oneshot, watch};

mod process;
mod progress;
use process::graceful_stop;
pub use process::resolve_ffmpeg_path;
pub(crate) use process::{ffmpeg_command, spawn_ffmpeg, NULL_SINK};
use progress::ProgressParser;

const MAX_RESTARTS: u32 = 3;
const RESTART_DELAY: Duration = Duration::from_secs(5);
const PROGRESS_STALL: Duration = Duration::from_secs(10);
const PRELIVE_TIMEOUT: Duration = Duration::from_secs(20);
const PRELIVE_TIMEOUT_HW: Duration = Duration::from_secs(8);

fn emit_encoder(app: &AppHandle, preferred: &str, encoder: Encoder) {
    let _ = app.emit(
        crate::events::STREAM_ENCODER,
        EncoderStatusPayload {
            preferred: preferred.to_string(),
            effective: encoder.name().to_string(),
        },
    );
}

#[cfg(windows)]
fn start_debug_preview(app: &AppHandle) -> Option<(String, tauri::async_runtime::JoinHandle<()>)> {
    match super::capture_pipe::new_preview_pipe() {
        Ok((name, server)) => Some((name, spawn_preview_reader(app.clone(), server))),
        Err(e) => {
            mlog!(LogCat::Stream, "[debug-preview] {e}");
            None
        }
    }
}

#[cfg(windows)]
fn spawn_preview_reader(
    app: AppHandle,
    server: tokio::net::windows::named_pipe::NamedPipeServer,
) -> tauri::async_runtime::JoinHandle<()> {
    tauri::async_runtime::spawn(async move {
        if server.connect().await.is_err() {
            return;
        }
        let mut reader = BufReader::new(server);
        while let Some(jpeg) = super::preview::read_mpjpeg_frame(&mut reader).await {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
            let _ = app.emit(
                crate::events::STREAM_PREVIEW,
                PreviewEvent::Frame { frame: b64 },
            );
        }
    })
}

#[cfg(not(windows))]
fn start_debug_preview(_app: &AppHandle) -> Option<(String, tauri::async_runtime::JoinHandle<()>)> {
    None
}

pub async fn supervise(
    app: AppHandle,
    state: SharedState,
    spec: LaunchSpec,
    stop_rx: watch::Receiver<bool>,
    live_tx: Option<oneshot::Sender<Result<(), String>>>,
) {
    let debug_stream = crate::settings::load_stream_settings(&app).debug_stream;
    run_supervisor(app, state, spec, stop_rx, live_tx, debug_stream).await;
}

async fn run_supervisor(
    app: AppHandle,
    state: SharedState,
    spec: LaunchSpec,
    mut stop_rx: watch::Receiver<bool>,
    mut live_tx: Option<oneshot::Sender<Result<(), String>>>,
    debug_stream: bool,
) {
    let LaunchSpec {
        ffmpeg_path,
        whip_url,
        settings,
        replay_base,
        mut encoder,
        mut fallback,
        preferred,
    } = spec;

    let mut ever_live = false;
    let mut attempt: u32 = 0;
    let mut segment: u32 = 0;

    emit_encoder(&app, &preferred, encoder);

    loop {
        let replay = replay_base
            .as_ref()
            .and_then(|b| super::replay_run(b, segment));
        let wgc =
            match super::capture::start_capture_for(&settings.source, settings.framerate.max(1))
                .await
            {
                Ok(h) => h,
                Err(e) => {
                    mlog!(LogCat::Stream, "[ffmpeg] capture failed: {e}");
                    emit_status(&app, StreamState::Error, Some(e));
                    clear_session(&state);
                    return;
                }
            };
        let audio = audio::start_audio();
        let video_pipe = wgc.as_ref().map(|w| pipeline::VideoPipe {
            path: w.pipe_name(),
            width: w.width(),
            height: w.height(),
        });
        let preview_pipe = debug_stream.then(|| start_debug_preview(&app)).flatten();
        let (preview_pipe_name, preview_reader) = match preview_pipe {
            Some((name, reader)) => (Some(name), Some(reader)),
            None => (None, None),
        };
        let args = match pipeline::build_args(
            &settings,
            &whip_url,
            &audio.source,
            replay.as_ref(),
            video_pipe.as_ref(),
            encoder,
            preview_pipe_name.as_deref(),
        ) {
            Ok(a) => a,
            Err(e) => {
                mlog!(LogCat::Stream, "[ffmpeg] bad pipeline args: {e}");
                audio.shutdown().await;
                if let Some(w) = wgc {
                    w.shutdown().await;
                }
                if let Some(r) = preview_reader {
                    r.abort();
                }
                emit_status(&app, StreamState::Error, Some(e));
                clear_session(&state);
                return;
            }
        };
        if let Some(r) = replay.as_ref() {
            mlog!(LogCat::Stream, "[replay] writing {}", r.pattern.display());
        }
        record_run_encoder(replay.as_ref(), segment, encoder);
        mlog!(LogCat::Stream, "[ffmpeg] spawn: {}", args.join(" "));

        let child = match spawn_ffmpeg(&ffmpeg_path, &args) {
            Ok(c) => c,
            Err(e) => {
                mlog!(LogCat::Stream, "[ffmpeg] spawn failed: {e}");
                audio.shutdown().await;
                if let Some(w) = wgc {
                    w.shutdown().await;
                }
                if let Some(r) = preview_reader {
                    r.abort();
                }
                emit_status(&app, StreamState::Error, Some(e));
                clear_session(&state);
                return;
            }
        };
        let replay_watch = replay.as_ref().map(|r| {
            let (tx, rx) = tokio::sync::watch::channel(false);
            let handle = tauri::async_runtime::spawn(super::replay::supervise_run(
                state.clone(),
                r.clone(),
                segment,
                rx,
            ));
            (tx, handle)
        });

        let prelive = if encoder == Encoder::X264 {
            PRELIVE_TIMEOUT
        } else {
            PRELIVE_TIMEOUT_HW
        };

        let (outcome, went_live, err_tail) =
            run_child(&app, child, &mut stop_rx, &mut live_tx, prelive, encoder).await;

        if let Some((tx, handle)) = replay_watch {
            let _ = tx.send(true);
            let _ = handle.await;
        }
        audio.shutdown().await;
        if let Some(w) = wgc {
            w.shutdown().await;
        }
        if let Some(r) = preview_reader {
            r.abort();
        }

        if went_live {
            ever_live = true;
        }

        match outcome {
            Outcome::Stopped => {
                emit_status(&app, StreamState::Stopped, None);
                return;
            }
            Outcome::Died => {
                if let Some(bad_args) = err_tail
                    .iter()
                    .find(|l| l.contains("Error splitting the argument list"))
                {
                    let msg = format!("ffmpeg rejected its own arguments ({bad_args}) — this is a bug, please report it");
                    mlog!(LogCat::Stream, "[ffmpeg] {msg}");
                    if let Some(tx) = live_tx.take() {
                        let _ = tx.send(Err(msg.clone()));
                    }
                    emit_status(&app, StreamState::Error, Some(msg));
                    clear_session(&state);
                    return;
                }
                if !ever_live && hw_encoder_failed(&err_tail) {
                    if let Some(next) = fallback.take() {
                        let reason = err_tail.last().cloned();
                        mlog!(
                            LogCat::Stream,
                            "[ffmpeg] {} unusable{}, falling back to {}",
                            encoder.name(),
                            reason.map(|r| format!(": {r}")).unwrap_or_default(),
                            next.name()
                        );
                        mark_mixed_encoders(replay.as_ref());
                        encoder = next;
                        emit_encoder(&app, &preferred, encoder);
                        emit_status(
                            &app,
                            StreamState::Connecting,
                            Some(encoder.name().to_string()),
                        );
                        segment += 1;
                        continue;
                    }
                    let reason = err_tail.last().cloned();
                    mlog!(
                        LogCat::Stream,
                        "[ffmpeg] {} unusable{}, no fallback left",
                        encoder.name(),
                        reason.clone().map(|r| format!(": {r}")).unwrap_or_default()
                    );
                    let msg = format!(
                        "{} couldn't start{} — pick a different encoder in settings",
                        encoder.name(),
                        reason.map(|r| format!(": {r}")).unwrap_or_default()
                    );
                    if let Some(tx) = live_tx.take() {
                        let _ = tx.send(Err(msg.clone()));
                    }
                    emit_status(&app, StreamState::Error, Some(msg));
                    clear_session(&state);
                    return;
                }
                if went_live {
                    attempt = 0;
                }
                let phase = state.lock_state().app_state.clone();

                if phase == AppState::RaceInProgress {
                    attempt += 1;
                    if attempt > MAX_RESTARTS {
                        mlog!(LogCat::Stream, "[ffmpeg] mid-race restarts exhausted");
                        emit_status(&app, StreamState::Error, Some("stream lost".into()));
                        clear_session(&state);
                        return;
                    }
                    mlog!(
                        LogCat::Stream,
                        "[ffmpeg] mid-race death, restart {attempt}/{MAX_RESTARTS}"
                    );
                    emit_status(&app, StreamState::Reconnecting, None);
                    if wait_or_stop(&mut stop_rx, RESTART_DELAY).await {
                        emit_status(&app, StreamState::Stopped, None);
                        return;
                    }
                    segment += 1;
                    continue;
                }

                mlog!(LogCat::Stream, "[ffmpeg] pre-race death");
                if matches!(phase, AppState::StreamSetup | AppState::WaitingForStart) {
                    let lobby_id = state
                        .lock_state()
                        .lobby
                        .as_ref()
                        .map(|l| l.lobby_id.clone());
                    if let Some(id) = lobby_id {
                        let _ = crate::api::lobby::post_stream_stopped(&app, &id).await;
                    }
                    state.lock_state().app_state = AppState::StreamSetup;
                    let _ = app.emit(crate::events::APP_STATE, AppState::StreamSetup);
                }
                emit_status(&app, StreamState::Error, Some("stream ended".into()));
                clear_session(&state);
                super::preview::ensure_for_phase(&app, &state);
                return;
            }
        }
    }
}

fn clear_session(state: &SharedState) {
    state.lock_state().stream = None;
}

fn hw_encoder_failed(tail: &[String]) -> bool {
    tail.iter().any(|l| {
        l.contains("No capable devices found")
            || l.contains("OpenEncodeSessionEx failed")
            || l.contains("Cannot load nvcuda")
            || l.contains("Cannot load nvEncodeAPI")
            || l.contains("amfrt64.dll")
            || l.contains("AMFCreateContext")
    })
}

fn mark_mixed_encoders(replay: Option<&ReplayRun>) {
    if let Some(r) = replay {
        let _ = std::fs::File::create(super::replay::mixed_encoders_path(&r.dir));
    }
}

fn record_run_encoder(replay: Option<&ReplayRun>, run: u32, enc: Encoder) {
    if let Some(r) = replay {
        let _ = std::fs::write(super::replay::encoder_path(&r.dir, run), enc.name());
    }
}

async fn wait_or_stop(stop_rx: &mut watch::Receiver<bool>, dur: Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(dur) => false,
        r = stop_rx.changed() => r.is_ok() && *stop_rx.borrow(),
    }
}

async fn run_child(
    app: &AppHandle,
    mut child: Child,
    stop_rx: &mut watch::Receiver<bool>,
    live_tx: &mut Option<oneshot::Sender<Result<(), String>>>,
    prelive_timeout: Duration,
    encoder: Encoder,
) -> (Outcome, bool, Vec<String>) {
    let spawned = Instant::now();
    let mut stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let last_err: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    let mut reader = None;
    if let Some(err) = stderr {
        let tail = last_err.clone();
        reader = Some(tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                mlog!(LogCat::Stream, "[ffmpeg] {l}");
                if let Ok(mut t) = tail.lock() {
                    t.push_back(l);
                    while t.len() > 40 {
                        t.pop_front();
                    }
                }
            }
        }));
    }

    let drain = |reader: Option<tauri::async_runtime::JoinHandle<()>>,
                 last_err: Arc<Mutex<VecDeque<String>>>| async move {
        if let Some(r) = reader {
            let _ = tokio::time::timeout(Duration::from_millis(500), r).await;
        }
        last_err
            .lock()
            .map(|t| t.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default()
    };

    let Some(stdout) = stdout else {
        let _ = child.kill().await;
        return (Outcome::Died, false, drain(reader, last_err).await);
    };
    let mut lines = BufReader::new(stdout).lines();
    let mut went_live = false;
    let mut last_progress = Instant::now();
    let mut stall = tokio::time::interval(Duration::from_secs(1));
    let mut parser = ProgressParser::default();
    let mut valid_blocks: u32 = 0;
    let mut last_frame: Option<u64> = None;
    let mut ticks_since_heartbeat: u32 = 0;

    loop {
        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_ok() && *stop_rx.borrow() {
                    graceful_stop(&mut child, &mut stdin).await;
                    return (Outcome::Stopped, went_live, drain(reader, last_err).await);
                }
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        let Some(block) = parser.feed(&l) else { continue };
                        last_progress = Instant::now();

                        let increasing = match block.frame {
                            Some(f) => last_frame.map(|prev| f > prev).unwrap_or(true),
                            None => false,
                        };
                        if increasing {
                            last_frame = block.frame;
                            valid_blocks += 1;
                        } else {
                            valid_blocks = 0;
                        }

                        if !went_live && valid_blocks >= 2 {
                            went_live = true;
                            mlog!(
                                LogCat::Stream,
                                "[ffmpeg] live: {} after {:?} (frame={})",
                                encoder.name(),
                                spawned.elapsed(),
                                last_frame.unwrap_or(0)
                            );
                            emit_status(app, StreamState::Live, None);
                            if let Some(tx) = live_tx.take() {
                                let _ = tx.send(Ok(()));
                            }
                        }
                    }
                    Ok(None) | Err(_) => {
                        let _ = child.wait().await;
                        return (Outcome::Died, went_live, drain(reader, last_err).await);
                    }
                }
            }
            _ = stall.tick() => {
                if went_live && last_progress.elapsed() > PROGRESS_STALL {
                    mlog!(LogCat::Stream, "[ffmpeg] progress stalled, killing");
                    let _ = child.kill().await;
                    return (Outcome::Died, went_live, drain(reader, last_err).await);
                }
                if went_live {
                    ticks_since_heartbeat += 1;
                    if ticks_since_heartbeat >= 5 {
                        ticks_since_heartbeat = 0;
                        mlog!(
                            LogCat::Stream,
                            "[ffmpeg] heartbeat: {} frame={}",
                            encoder.name(),
                            last_frame.unwrap_or(0)
                        );
                    }
                }
                if !went_live && spawned.elapsed() > prelive_timeout {
                    mlog!(LogCat::Stream, "[ffmpeg] never went live, killing");
                    let _ = child.kill().await;
                    return (Outcome::Died, went_live, drain(reader, last_err).await);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
