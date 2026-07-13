use super::{ffmpeg, pipeline, PreviewEvent, PreviewSession};
use crate::events::STREAM_PREVIEW;
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::SharedState;
use base64::Engine;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::sync::watch;

static PREVIEW_ID: AtomicU64 = AtomicU64::new(1);

fn emit(app: &AppHandle, ev: PreviewEvent) {
    let _ = app.emit(STREAM_PREVIEW, ev);
}

pub fn ensure_for_phase(app: &AppHandle, state: &SharedState) {
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = start(&app, &state).await {
            mlog!(LogCat::Stream, "[preview] auto-start failed: {e}");
        }
    });
}

pub async fn start(app: &AppHandle, state: &SharedState) -> Result<(), String> {
    let res = start_inner(app, state).await;
    if let Err(e) = &res {
        emit(app, PreviewEvent::Error { error: e.clone() });
    }
    res
}

async fn start_inner(app: &AppHandle, state: &SharedState) -> Result<(), String> {
    {
        let guard = state.lock().map_err(|e| e.to_string())?;
        if guard.app_state != AppState::StreamSetup {
            return Ok(());
        }
        if guard.preview.is_some() {
            return Ok(());
        }
        if guard.stream.is_some() {
            return Err("cannot preview while the stream is live".into());
        }
    }

    let source = super::current_source(app, state);
    let ffmpeg_path = ffmpeg::resolve_ffmpeg_path()?;
    let wgc = match &source {
        super::CaptureSource::Window { hwnd, .. } => Some(super::wgc::start_window_capture(
            *hwnd,
            pipeline::PREVIEW_FPS,
        )?),
        _ => None,
    };
    let video_pipe = wgc.as_ref().map(|w| pipeline::VideoPipe {
        path: &w.pipe_name,
        width: w.width,
        height: w.height,
    });
    let args = match pipeline::build_preview_args(&source, video_pipe.as_ref()) {
        Ok(a) => a,
        Err(e) => {
            if let Some(w) = wgc {
                w.shutdown().await;
            }
            return Err(e);
        }
    };
    mlog!(LogCat::Stream, "[preview] spawn: {}", args.join(" "));

    let id = PREVIEW_ID.fetch_add(1, Ordering::Relaxed);
    let (stop_tx, stop_rx) = watch::channel(false);
    let app_c = app.clone();
    let state_c = state.clone();
    let join = tauri::async_runtime::spawn(async move {
        drive_preview(&app_c, &state_c, id, ffmpeg_path, args, stop_rx).await;
        if let Some(w) = wgc {
            w.shutdown().await;
        }
    });

    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if guard.preview.is_some() {
        let _ = stop_tx.send(true);
        return Ok(());
    }
    guard.preview = Some(PreviewSession { id, stop_tx, join });
    Ok(())
}

pub async fn stop(state: &SharedState) {
    let session = match state.lock() {
        Ok(mut g) => g.preview.take(),
        Err(_) => return,
    };
    let Some(session) = session else { return };
    let _ = session.stop_tx.send(true);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(2), session.join).await;
    mlog!(LogCat::Stream, "[preview] stopped");
}

pub async fn restart(app: &AppHandle, state: &SharedState) -> Result<(), String> {
    stop(state).await;
    start(app, state).await
}

async fn drive_preview(
    app: &AppHandle,
    state: &SharedState,
    id: u64,
    ffmpeg_path: std::path::PathBuf,
    args: Vec<String>,
    mut stop_rx: watch::Receiver<bool>,
) {
    let mut child = match ffmpeg::spawn_ffmpeg(&ffmpeg_path, &args) {
        Ok(c) => c,
        Err(e) => {
            mlog!(LogCat::Stream, "[preview] spawn failed: {e}");
            emit(app, PreviewEvent::Error { error: e });
            clear_own_session(state, id);
            return;
        }
    };

    if let Some(err) = child.stderr.take() {
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                mlog!(LogCat::Stream, "[preview:ffmpeg] {l}");
            }
        });
    }

    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill().await;
        emit(
            app,
            PreviewEvent::Error {
                error: "preview has no stdout".into(),
            },
        );
        clear_own_session(state, id);
        return;
    };
    let mut reader = BufReader::new(stdout);

    loop {
        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return;
                }
            }
            frame = read_mpjpeg_frame(&mut reader) => {
                match frame {
                    Some(jpeg) => {
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&jpeg);
                        if let Ok(mut g) = state.lock() {
                            g.preview_last_jpeg = Some(jpeg);
                        }
                        emit(app, PreviewEvent::Frame { frame: b64 });
                    }
                    // stdout closed => ffmpeg exited
                    None => {
                        let _ = child.wait().await;
                        mlog!(LogCat::Stream, "[preview] ffmpeg ended unexpectedly");
                        emit(app, PreviewEvent::Error { error: "preview ended".into() });
                        clear_own_session(state, id);
                        return;
                    }
                }
            }
        }
    }
}

fn clear_own_session(state: &SharedState, id: u64) {
    if let Ok(mut g) = state.lock() {
        if g.preview.as_ref().map(|p| p.id) == Some(id) {
            g.preview = None;
        }
    }
}

async fn read_mpjpeg_frame<R: tokio::io::AsyncBufRead + Unpin>(reader: &mut R) -> Option<Vec<u8>> {
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await.ok()?;
        if n == 0 {
            return None; // EOF
        }
        let line = line.trim();
        if let Some(v) = line
            .to_ascii_lowercase()
            .strip_prefix("content-length:")
            .map(|v| v.trim().to_string())
        {
            content_length = v.parse().ok();
        } else if line.is_empty() {
            if let Some(len) = content_length.take() {
                // 8 MB cap
                if len == 0 || len > 8 * 1024 * 1024 {
                    return None;
                }
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf).await.ok()?;
                return Some(buf);
            }
        }
    }
}
