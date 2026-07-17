use crate::state::SharedState;
use base64::Engine;
use tauri::{AppHandle, State};

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tauri::command]
pub async fn capture_monitor_thumb(
    index: u32,
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<String, String> {
    let on_this_monitor = matches!(
        super::current_source(&app, &state),
        super::CaptureSource::Monitor { index: i } if i == index
    );
    let reuse = if on_this_monitor {
        state
            .lock()
            .map_err(|e| e.to_string())?
            .preview_last_jpeg
            .clone()
    } else {
        None
    };
    if let Some(jpeg) = reuse {
        return Ok(b64(&jpeg));
    }

    let ffmpeg_path = super::ffmpeg::resolve_ffmpeg_path()?;
    let args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        &format!("ddagrab=output_idx={index}:framerate=5"),
        "-frames:v",
        "1",
        "-vf",
        "hwdownload,format=bgra,scale=320:-2:flags=bilinear,format=yuvj420p",
        "-q:v",
        "8",
        "-f",
        "mjpeg",
        "pipe:1",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut child = super::ffmpeg::spawn_ffmpeg(&ffmpeg_path, &args)?;
    let mut stdout = child.stdout.take().ok_or("thumb ffmpeg has no stdout")?;
    let mut jpeg = Vec::new();
    let read = tokio::time::timeout(std::time::Duration::from_secs(4), async {
        use tokio::io::AsyncReadExt;
        stdout.read_to_end(&mut jpeg).await
    })
    .await;
    let _ = child.kill().await;
    let _ = child.wait().await;
    match read {
        Ok(Ok(_)) if !jpeg.is_empty() => Ok(b64(&jpeg)),
        _ => Err("monitor thumbnail capture failed".into()),
    }
}

#[cfg(windows)]
#[tauri::command]
pub async fn capture_window_thumb(hwnd: u64) -> Result<String, String> {
    use windows_capture::capture::GraphicsCaptureApiHandler;
    use windows_capture::settings::{
        ColorFormat, CursorCaptureSettings, DirtyRegionSettings, DrawBorderSettings,
        MinimumUpdateIntervalSettings, SecondaryWindowSettings, Settings,
    };
    use windows_capture::window::Window;

    static SEM: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let _permit = SEM.acquire().await.map_err(|e| e.to_string())?;

    let window = Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void);
    if !window.is_valid() {
        return Err("window is gone".into());
    }

    struct ThumbCapture {
        tx: Option<std::sync::mpsc::Sender<Result<Vec<u8>, String>>>,
    }
    impl GraphicsCaptureApiHandler for ThumbCapture {
        type Flags = std::sync::mpsc::Sender<Result<Vec<u8>, String>>;
        type Error = Box<dyn std::error::Error + Send + Sync>;

        fn new(ctx: windows_capture::capture::Context<Self::Flags>) -> Result<Self, Self::Error> {
            Ok(Self {
                tx: Some(ctx.flags),
            })
        }

        fn on_frame_arrived(
            &mut self,
            frame: &mut windows_capture::frame::Frame,
            capture_control: windows_capture::graphics_capture_api::InternalCaptureControl,
        ) -> Result<(), Self::Error> {
            if let Some(tx) = self.tx.take() {
                let path = std::env::temp_dir().join(format!(
                    "speedrace_thumb_{:016x}.jpg",
                    rand::random::<u64>()
                ));
                let res = frame
                    .save_as_image(&path, windows_capture::encoder::ImageFormat::Jpeg)
                    .map_err(|e| e.to_string())
                    .and_then(|()| std::fs::read(&path).map_err(|e| e.to_string()));
                let _ = std::fs::remove_file(&path);
                let _ = tx.send(res);
                capture_control.stop();
            }
            Ok(())
        }
    }

    let (tx, rx) = std::sync::mpsc::channel::<Result<Vec<u8>, String>>();
    let settings = Settings::new(
        window,
        CursorCaptureSettings::WithoutCursor,
        DrawBorderSettings::WithoutBorder,
        SecondaryWindowSettings::Default,
        MinimumUpdateIntervalSettings::Default,
        DirtyRegionSettings::Default,
        ColorFormat::Bgra8,
        tx,
    );
    let control = ThumbCapture::start_free_threaded(settings).map_err(|e| e.to_string())?;

    let jpeg = tokio::task::spawn_blocking(move || {
        let res = rx
            .recv_timeout(std::time::Duration::from_secs(3))
            .map_err(|_| "window thumbnail timed out".to_string())?;
        let _ = control.stop();
        res
    })
    .await
    .map_err(|e| e.to_string())??;

    Ok(b64(&jpeg))
}

#[cfg(not(windows))]
#[tauri::command]
pub async fn capture_window_thumb(_hwnd: u64) -> Result<String, String> {
    Err("window capture is only supported on Windows".into())
}
