use super::WgcHandle;
#[cfg(windows)]
use super::{WgcCapture, WgcError, WgcFlags};
#[cfg(windows)]
use crate::logging::{mlog, LogCat};

impl WgcHandle {
    pub async fn shutdown(self) {
        #[cfg(windows)]
        {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(w) = self.writer {
                w.abort();
                let _ = w.await;
            }
            if let Some(c) = self.control {
                let _ = tokio::task::spawn_blocking(move || c.stop()).await;
            }
            mlog!(LogCat::Stream, "[wgc] capture stopped");
        }
    }
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
            last_dims: (0, 0),
        })
    }

    fn on_frame_arrived(
        &mut self,
        frame: &mut windows_capture::frame::Frame,
        _capture_control: windows_capture::graphics_capture_api::InternalCaptureControl,
    ) -> Result<(), Self::Error> {
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
        Ok(())
    }

    fn on_closed(&mut self) -> Result<(), Self::Error> {
        self.closed.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(windows)]
pub fn start_window_capture(hwnd: u64, fps: u32) -> Result<WgcHandle, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::io::AsyncWriteExt;
    use tokio::net::windows::named_pipe::ServerOptions;
    use windows_capture::capture::GraphicsCaptureApiHandler;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };
    use windows_capture::window::Window;

    let window = Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void);
    if !window.is_valid() {
        return Err("window is gone; pick another source".into());
    }
    // rawvideo needs one fixed size: lock it to the window rect at start (even dims)
    let w = window.width().map_err(|e| e.to_string())?.max(2) as u32 & !1;
    let h = window.height().map_err(|e| e.to_string())?.max(2) as u32 & !1;

    let pipe_name = format!(r"\\.\pipe\momentum_video_{:016x}", rand::random::<u64>());
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)
        .map_err(|e| format!("video pipe create failed: {e}"))?;

    let latest: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(vec![0u8; (w * h * 4) as usize]));
    let closed: super::StopFlag = Arc::new(AtomicBool::new(false));
    let stop: super::StopFlag = Arc::new(AtomicBool::new(false));

    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        WgcFlags {
            target_w: w,
            target_h: h,
            latest: latest.clone(),
            closed: closed.clone(),
        },
    );
    let control = WgcCapture::start_free_threaded(settings).map_err(|e| e.to_string())?;

    let frame_bytes = (w * h * 4) as usize;
    let period = std::time::Duration::from_millis((1000 / fps.max(1)).max(1) as u64);
    let stop_w = stop.clone();
    let writer = tauri::async_runtime::spawn(async move {
        // wait for ffmpeg
        if server.connect().await.is_err() {
            return;
        }
        let mut server = server;
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut out = vec![0u8; frame_bytes];
        loop {
            ticker.tick().await;
            if stop_w.load(Ordering::SeqCst) || closed.load(Ordering::SeqCst) {
                return; // dropping  EOFs ffmpeg video input
            }
            if let Ok(l) = latest.lock() {
                out.copy_from_slice(&l);
            }
            if server.write_all(&out).await.is_err() {
                return;
            }
        }
    });

    mlog!(
        LogCat::Stream,
        "[wgc] capturing hwnd={hwnd:#x} {w}x{h} @ {fps}fps, pipe {pipe_name}"
    );
    Ok(WgcHandle {
        pipe_name,
        width: w,
        height: h,
        control: Some(control),
        writer: Some(writer),
        stop,
    })
}

#[cfg(not(windows))]
pub fn start_window_capture(_hwnd: u64, _fps: u32) -> Result<WgcHandle, String> {
    Err("window capture is only supported on Windows".into())
}
