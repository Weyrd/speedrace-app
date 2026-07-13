use super::pipeline;
use super::{audio, emit_status, LaunchSpec, Outcome, StreamState};
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::SharedState;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::watch;

const MAX_RESTARTS: u32 = 3;
const RESTART_DELAY: Duration = Duration::from_secs(5);
const PROGRESS_STALL: Duration = Duration::from_secs(10);
// A WHIP handshake that never yields a progress block must not hang forever
const PRELIVE_TIMEOUT: Duration = Duration::from_secs(20);

pub fn resolve_ffmpeg_path() -> Result<PathBuf, String> {
    let exe_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let c = dir.join(exe_name);
            if c.exists() {
                return Ok(c);
            }
        }
    }
    // Dev => externalBin may not be copied next to the debug exe
    #[cfg(debug_assertions)]
    {
        let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
        if let Ok(rd) = std::fs::read_dir(&bin) {
            for e in rd.flatten() {
                let n = e.file_name();
                let n = n.to_string_lossy();
                if n.starts_with("ffmpeg") && n.ends_with(".exe") {
                    return Ok(e.path());
                }
            }
        }
    }
    Err("ffmpeg sidecar not found; run src-tauri/scripts/get-ffmpeg.ps1".into())
}

pub async fn supervise(
    app: AppHandle,
    state: SharedState,
    spec: LaunchSpec,
    mut stop_rx: watch::Receiver<bool>,
    mut live_tx: Option<tokio::sync::oneshot::Sender<()>>,
) {
    let LaunchSpec {
        ffmpeg_path,
        whip_url,
        settings,
        replay_base,
    } = spec;
    let mut attempt: u32 = 0;
    let mut segment: u32 = 0;

    loop {
        let replay = replay_base
            .as_ref()
            .map(|b| super::segment_path(b, segment));
        // window sources
        let wgc = match &settings.source {
            super::CaptureSource::Window { hwnd, .. } => {
                match super::wgc::start_window_capture(*hwnd, settings.framerate.max(1)) {
                    Ok(h) => Some(h),
                    Err(e) => {
                        mlog!(LogCat::Stream, "[ffmpeg] window capture failed: {e}");
                        emit_status(&app, StreamState::Error, Some(e));
                        clear_session(&state);
                        return;
                    }
                }
            }
            _ => None,
        };
        let audio = audio::start_audio();
        let video_pipe = wgc.as_ref().map(|w| pipeline::VideoPipe {
            path: &w.pipe_name,
            width: w.width,
            height: w.height,
        });
        let args = match pipeline::build_args(
            &settings,
            &whip_url,
            &audio.source,
            replay.as_deref(),
            video_pipe.as_ref(),
        ) {
            Ok(a) => a,
            Err(e) => {
                mlog!(LogCat::Stream, "[ffmpeg] bad pipeline args: {e}");
                audio.shutdown().await;
                if let Some(w) = wgc {
                    w.shutdown().await;
                }
                emit_status(&app, StreamState::Error, Some(e));
                clear_session(&state);
                return;
            }
        };
        if let Some(p) = replay.as_ref() {
            mlog!(LogCat::Stream, "[replay] writing {}", p.display());
        }
        mlog!(LogCat::Stream, "[ffmpeg] spawn: {}", args.join(" "));

        let child = match spawn_ffmpeg(&ffmpeg_path, &args) {
            Ok(c) => c,
            Err(e) => {
                mlog!(LogCat::Stream, "[ffmpeg] spawn failed: {e}");
                audio.shutdown().await;
                if let Some(w) = wgc {
                    w.shutdown().await;
                }
                emit_status(&app, StreamState::Error, Some(e));
                clear_session(&state);
                return;
            }
        };

        let (outcome, went_live) = run_child(&app, child, &mut stop_rx, &mut live_tx).await;
        audio.shutdown().await;
        if let Some(w) = wgc {
            w.shutdown().await;
        }

        match outcome {
            Outcome::Stopped => {
                emit_status(&app, StreamState::Stopped, None);
                return;
            }
            Outcome::Died => {
                if went_live {
                    attempt = 0;
                }
                let phase = state
                    .lock()
                    .map(|g| g.app_state.clone())
                    .unwrap_or(AppState::Idle);

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
                    // Never POST stream-stopped mid-race: the back would forfeit the runner.
                    if wait_or_stop(&mut stop_rx, RESTART_DELAY).await {
                        emit_status(&app, StreamState::Stopped, None);
                        return;
                    }
                    segment += 1;
                    continue;
                }

                // Pre race death is safe to surface reset ready flags and return to setup
                mlog!(LogCat::Stream, "[ffmpeg] pre-race death");
                if matches!(phase, AppState::StreamSetup | AppState::WaitingForStart) {
                    let lobby_id = state
                        .lock()
                        .ok()
                        .and_then(|g| g.lobby.as_ref().map(|l| l.lobby_id.clone()));
                    if let Some(id) = lobby_id {
                        let _ = crate::api::lobby::post_stream_stopped(&app, &id).await;
                    }
                    if let Ok(mut g) = state.lock() {
                        g.app_state = AppState::StreamSetup;
                    }
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
    if let Ok(mut g) = state.lock() {
        g.stream = None;
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
    live_tx: &mut Option<tokio::sync::oneshot::Sender<()>>,
) -> (Outcome, bool) {
    let spawned = Instant::now();
    let mut stdin = child.stdin.take();
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let last_err: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
    if let Some(err) = stderr {
        let tail = last_err.clone();
        tauri::async_runtime::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(l)) = lines.next_line().await {
                mlog!(LogCat::Stream, "[ffmpeg] {l}");
                if let Ok(mut t) = tail.lock() {
                    t.push_back(l);
                    while t.len() > 20 {
                        t.pop_front();
                    }
                }
            }
        });
    }

    let Some(stdout) = stdout else {
        let _ = child.kill().await;
        return (Outcome::Died, false);
    };
    let mut lines = BufReader::new(stdout).lines();
    let mut went_live = false;
    let mut last_progress = Instant::now();
    let mut stall = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            changed = stop_rx.changed() => {
                if changed.is_ok() && *stop_rx.borrow() {
                    graceful_stop(&mut child, &mut stdin).await;
                    return (Outcome::Stopped, went_live);
                }
            }
            line = lines.next_line() => {
                match line {
                    Ok(Some(l)) => {
                        if l.starts_with("progress=") {
                            last_progress = Instant::now();
                            if !went_live {
                                went_live = true;
                                emit_status(app, StreamState::Live, None);
                                if let Some(tx) = live_tx.take() {
                                    let _ = tx.send(());
                                }
                            }
                        }
                    }
                    // stdout closed => ffmpeg exited.
                    Ok(None) | Err(_) => {
                        let _ = child.wait().await;
                        return (Outcome::Died, went_live);
                    }
                }
            }
            _ = stall.tick() => {
                if went_live && last_progress.elapsed() > PROGRESS_STALL {
                    mlog!(LogCat::Stream, "[ffmpeg] progress stalled, killing");
                    let _ = child.kill().await;
                    return (Outcome::Died, went_live);
                }
                if !went_live && spawned.elapsed() > PRELIVE_TIMEOUT {
                    mlog!(LogCat::Stream, "[ffmpeg] never went live, killing");
                    let _ = child.kill().await;
                    return (Outcome::Died, went_live);
                }
            }
        }
    }
}

async fn graceful_stop(child: &mut Child, stdin: &mut Option<tokio::process::ChildStdin>) {
    if let Some(si) = stdin.as_mut() {
        let _ = si.write_all(b"q\n").await;
        let _ = si.flush().await;
    }
    *stdin = None; // drop to signal EOF
    if tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}

pub(crate) fn spawn_ffmpeg(path: &PathBuf, args: &[String]) -> Result<Child, String> {
    let mut cmd = Command::new(path);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW (tokio Command inherent on Windows)
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    assign_to_job(&child);
    Ok(child)
}

// Process-lifetime Job Object: if our process dies  Windows kills any ffmpeg spawned
#[cfg(windows)]
fn assign_to_job(child: &Child) {
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    static JOB: OnceLock<isize> = OnceLock::new();
    let handle = *JOB.get_or_init(|| unsafe {
        let Ok(h) = CreateJobObjectW(None, windows::core::PCWSTR::null()) else {
            return 0;
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let _ = SetInformationJobObject(
            h,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        h.0 as isize
    });

    if handle == 0 {
        return;
    }
    if let Some(raw) = child.raw_handle() {
        unsafe {
            let _ = AssignProcessToJobObject(HANDLE(handle as *mut _), HANDLE(raw as *mut _));
        }
    }
}
