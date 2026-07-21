use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use crate::logging::{mlog, LogCat};

const START_TIMEOUT: Duration = Duration::from_secs(10);
const HANDLE_RECHECK: Duration = Duration::from_millis(250);

pub struct GameCaptureHandle {
    pub pipe_name: String,
    pub width: u32,
    pub height: u32,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) capture: Option<std::thread::JoinHandle<()>>,
    pub(crate) writer: Option<tauri::async_runtime::JoinHandle<()>>,
}

impl GameCaptureHandle {
    pub async fn shutdown(self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(w) = self.writer {
            w.abort();
            let _ = w.await;
        }
        if let Some(c) = self.capture {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = c.join();
            })
            .await;
        }
        mlog!(LogCat::Stream, "[gc] capture stopped");
    }
}

pub(crate) async fn start(
    hwnd: u64,
    w: u32,
    h: u32,
    fps: u32,
) -> Result<GameCaptureHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let primed = Arc::new(AtomicBool::new(false));
    let latest: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(vec![0u8; w as usize * h as usize * 4]));
    let (tx, rx) = oneshot::channel::<Result<(), String>>();

    let (stop_c, primed_c, latest_c) = (stop.clone(), primed.clone(), latest.clone());
    let capture = std::thread::spawn(move || {
        capture_thread(hwnd, fps, w, h, stop_c, primed_c, latest_c, tx);
    });

    // wait only for injection
    match tokio::time::timeout(START_TIMEOUT, rx).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(e))) => {
            stop.store(true, Ordering::SeqCst);
            return Err(e);
        }
        Ok(Err(_)) => {
            stop.store(true, Ordering::SeqCst);
            return Err("game-capture ended before arming".into());
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            return Err("game-capture injection timed out".into());
        }
    }

    let frame_bytes = w as usize * h as usize * 4;
    let (pipe_name, server) = crate::stream::capture_pipe::new_video_pipe().inspect_err(|_| {
        stop.store(true, Ordering::SeqCst);
    })?;

    let writer = crate::stream::capture_pipe::spawn_paced_writer(
        server,
        latest,
        stop.clone(),
        primed,
        fps,
        frame_bytes,
    );
    mlog!(
        LogCat::Stream,
        "[gc] armed hwnd={hwnd:#x}, pipe {w}x{h} @ {fps}fps {pipe_name}; awaiting game frames"
    );
    Ok(GameCaptureHandle {
        pipe_name,
        width: w,
        height: h,
        stop,
        capture: Some(capture),
        writer: Some(writer),
    })
}

#[allow(clippy::too_many_arguments)]
fn capture_thread(
    hwnd: u64,
    fps: u32,
    w: u32,
    h: u32,
    stop: Arc<AtomicBool>,
    primed: Arc<AtomicBool>,
    latest: Arc<Mutex<Vec<u8>>>,
    tx: oneshot::Sender<Result<(), String>>,
) {
    let armed = match super::session::inject_and_arm(hwnd, fps) {
        Ok(a) => a,
        Err(e) => {
            let _ = tx.send(Err(e));
            return;
        }
    };
    if tx.send(Ok(())).is_err() {
        armed.release();
        return;
    }

    let mut reader = match super::frame::SharedTextureReader::new() {
        Ok(r) => r,
        Err(e) => {
            mlog!(LogCat::Stream, "[gc] D3D11 device failed: {e}");
            armed.release();
            return;
        }
    };
    let period = Duration::from_millis((1000 / fps.max(1)).max(1) as u64);
    let max_fails = (2000 / period.as_millis().max(1)) as u32 + 1;
    let mut fails = 0u32;
    let mut buf = vec![0u8; w as usize * h as usize * 4];
    let mut recheck = Instant::now() - Duration::from_secs(1);
    while !stop.load(Ordering::SeqCst) {
        let t = Instant::now();
        if reader.current_handle() == 0 || recheck.elapsed() >= HANDLE_RECHECK {
            recheck = Instant::now();
            if let Some((tex, _, _)) = armed.try_texture(hwnd) {
                if tex != reader.current_handle() {
                    match reader.open_texture(tex) {
                        Ok((tw, th)) => {
                            mlog!(LogCat::Stream, "[gc] texture {tw}x{th} (handle {tex:#x})");
                            fails = 0;
                        }
                        Err(e) => mlog!(LogCat::Stream, "[gc] open shared texture failed: {e}"),
                    }
                }
            }
        }
        if reader.current_handle() == 0 {
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        match reader.read_into(&mut buf, w, h) {
            Ok(()) => {
                fails = 0;
                if let Ok(mut l) = latest.lock() {
                    if l.len() == buf.len() {
                        l.copy_from_slice(&buf);
                    }
                }
                primed.store(true, Ordering::SeqCst);
            }
            Err(_) => {
                fails += 1;
                if fails >= max_fails {
                    mlog!(LogCat::Stream, "[gc] texture read failing; re-acquiring");
                    reader.clear();
                    fails = 0;
                }
            }
        }
        let el = t.elapsed();
        if el < period {
            std::thread::sleep(period - el);
        }
    }
    armed.release();
}
