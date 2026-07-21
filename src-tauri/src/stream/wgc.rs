use super::WgcHandle;
#[cfg(windows)]
use super::{WgcCapture, WgcError, WgcFlags};
#[cfg(windows)]
use crate::logging::{mlog, LogCat};

#[cfg(windows)]
const FRAME_STALE: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(windows)]
const SESSION_MIN_AGE: std::time::Duration = std::time::Duration::from_secs(3);

impl WgcHandle {
    pub async fn shutdown(self) {
        #[cfg(windows)]
        {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(w) = self.writer {
                w.abort();
                let _ = w.await;
            }
            if let Some(s) = self.session {
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), s).await;
            }
            mlog!(LogCat::Stream, "[wgc] capture stopped");
        }
    }
}

#[cfg(windows)]
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(windows)]
impl windows_capture::capture::GraphicsCaptureApiHandler for WgcCapture {
    type Flags = WgcFlags;
    type Error = WgcError;

    fn new(ctx: windows_capture::capture::Context<Self::Flags>) -> Result<Self, Self::Error> {
        let f = ctx.flags;
        Ok(Self {
            target_w: f.target_w,
            target_h: f.target_h,
            latest: f.latest,
            closed: f.closed,
            primed: f.primed,
            last_frame_ms: f.last_frame_ms,
            last_dims: (0, 0),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut windows_capture::frame::Frame,
        _capture_control: windows_capture::graphics_capture_api::InternalCaptureControl,
    ) -> Result<(), Self::Error> {
        self.last_frame_ms
            .store(now_ms(), std::sync::atomic::Ordering::SeqCst);
        let mut buf = frame.buffer()?;
        let (fw, fh, pitch) = (buf.width(), buf.height(), buf.row_pitch());
        let src = buf.as_raw_buffer();

        let Ok(mut latest) = self.latest.lock() else {
            return Ok(());
        };
        if self.last_dims != (fw, fh) {
            latest.fill(0);
            self.last_dims = (fw, fh);
        }

        let copy_w = fw.min(self.target_w) as usize;
        let copy_h = fh.min(self.target_h) as usize;
        let src_x = (fw as usize).saturating_sub(copy_w) / 2;
        let src_y = (fh as usize).saturating_sub(copy_h) / 2;
        let dst_x = (self.target_w as usize).saturating_sub(copy_w) / 2;
        let dst_y = (self.target_h as usize).saturating_sub(copy_h) / 2;
        let dst_stride = self.target_w as usize * 4;
        let src_stride = pitch as usize;

        for row in 0..copy_h {
            let s = (src_y + row) * src_stride + src_x * 4;
            let d = (dst_y + row) * dst_stride + dst_x * 4;
            let (Some(srow), Some(drow)) = (
                src.get(s..s + copy_w * 4),
                latest.get_mut(d..d + copy_w * 4),
            ) else {
                break;
            };
            drow.copy_from_slice(srow);
        }
        self.primed.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(windows)]
#[derive(Clone, Copy)]
pub(crate) enum CaptureTarget {
    Window { hwnd: u64 },
    Monitor { hmonitor: isize },
}

#[cfg(windows)]
fn start_session(
    target: CaptureTarget,
    flags: WgcFlags,
) -> Result<windows_capture::capture::CaptureControl<WgcCapture, WgcError>, String> {
    use windows_capture::capture::GraphicsCaptureApiHandler;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };

    macro_rules! start {
        ($item:expr) => {
            WgcCapture::start_free_threaded(Settings::new(
                $item,
                CursorCaptureSettings::WithCursor,
                DrawBorderSettings::WithoutBorder,
                SecondaryWindowSettings::Default,
                MinimumUpdateIntervalSettings::Default,
                DirtyRegionSettings::Default,
                ColorFormat::Bgra8,
                flags,
            ))
            .map_err(|e| e.to_string())
        };
    }

    match target {
        CaptureTarget::Window { hwnd } => {
            let window =
                windows_capture::window::Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void);
            if !window.is_valid() {
                return Err("window is gone; pick another source".into());
            }
            start!(window)
        }
        CaptureTarget::Monitor { hmonitor } => start!(
            windows_capture::monitor::Monitor::from_raw_hmonitor(hmonitor as *mut std::ffi::c_void)
        ),
    }
}

#[cfg(windows)]
pub(crate) fn start_capture(
    target: CaptureTarget,
    w: u32,
    h: u32,
    fps: u32,
) -> Result<WgcHandle, String> {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    let (pipe_name, server) = super::capture_pipe::new_video_pipe()?;

    let latest: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(vec![0u8; (w * h * 4) as usize]));
    let stop: super::StopFlag = Arc::new(AtomicBool::new(false));
    let primed: super::StopFlag = Arc::new(AtomicBool::new(false));
    let flags = WgcFlags {
        target_w: w,
        target_h: h,
        latest: latest.clone(),
        closed: Arc::new(AtomicBool::new(false)),
        primed: primed.clone(),
        last_frame_ms: Arc::new(AtomicU64::new(now_ms())),
    };

    let first_closed = flags.closed.clone();
    let control = start_session(target, flags.clone())?;

    let stop_s = stop.clone();
    let flags_s = flags.clone();
    let session = tauri::async_runtime::spawn(async move {
        let mut control = Some(control);
        let mut closed = first_closed;
        let mut started = tokio::time::Instant::now();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if stop_s.load(Ordering::SeqCst) {
                break;
            }
            let stale = now_ms().saturating_sub(flags_s.last_frame_ms.load(Ordering::SeqCst))
                > FRAME_STALE.as_millis() as u64;
            let dead = closed.load(Ordering::SeqCst) || control.is_none();
            if !(dead || (stale && started.elapsed() > SESSION_MIN_AGE)) {
                continue;
            }
            if let Some(c) = control.take() {
                let _ = tokio::task::spawn_blocking(move || c.stop()).await;
            }
            if stop_s.load(Ordering::SeqCst) {
                break;
            }
            mlog!(
                LogCat::Stream,
                "[wgc] session {}; recreating",
                if dead { "closed" } else { "stale" }
            );
            closed = Arc::new(AtomicBool::new(false));
            flags_s.last_frame_ms.store(now_ms(), Ordering::SeqCst);
            match start_session(
                target,
                WgcFlags {
                    closed: closed.clone(),
                    ..flags_s.clone()
                },
            ) {
                Ok(c) => {
                    control = Some(c);
                    started = tokio::time::Instant::now();
                }
                Err(e) => {
                    mlog!(LogCat::Stream, "[wgc] recreate failed: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
        if let Some(c) = control.take() {
            let _ = tokio::task::spawn_blocking(move || c.stop()).await;
        }
    });

    let frame_bytes = (w * h * 4) as usize;
    let writer = super::capture_pipe::spawn_paced_writer(
        server,
        latest,
        stop.clone(),
        primed.clone(),
        fps,
        frame_bytes,
    );

    let label = match target {
        CaptureTarget::Window { hwnd } => format!("hwnd={hwnd:#x}"),
        CaptureTarget::Monitor { hmonitor } => format!("hmonitor={hmonitor:#x}"),
    };
    mlog!(
        LogCat::Stream,
        "[wgc] capturing {label} {w}x{h} @ {fps}fps, pipe {pipe_name}"
    );
    Ok(WgcHandle {
        pipe_name,
        width: w,
        height: h,
        session: Some(session),
        writer: Some(writer),
        stop,
        primed,
    })
}
