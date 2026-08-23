use crate::logging;
use crate::state::SharedState;
use crate::stream;
use std::fmt::Write;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn collect_debug_report(
    state: State<'_, SharedState>,
    app: AppHandle,
) -> Result<String, String> {
    let mut out = String::new();

    write_header(&mut out, &app);
    write_state(&mut out, &state);
    write_environment(&mut out, &app);
    write_capture(&mut out, &app, &state);
    write_log(&mut out);

    Ok(out)
}

fn write_header(out: &mut String, app: &AppHandle) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let version = app.package_info().version.to_string();
    let build = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let speedrace_log = std::env::var("SPEEDRACE_LOG").unwrap_or_else(|_| "(unset)".into());
    let _ = writeln!(out, "=== Speedrace debug report ===");
    let _ = writeln!(out, "generated: {now}");
    let _ = writeln!(out, "version: {version} ({build})");
    let _ = writeln!(
        out,
        "platform: {}/{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let _ = writeln!(out, "SPEEDRACE_LOG: {speedrace_log}");
    let _ = writeln!(out);
}

fn write_state(out: &mut String, state: &SharedState) {
    let _ = writeln!(out, "=== App state ===");
    let Ok(g) = state.lock() else {
        let _ = writeln!(out, "(state mutex poisoned)");
        let _ = writeln!(out);
        return;
    };
    let _ = writeln!(out, "app_state: {:?}", g.app_state);
    let _ = writeln!(out, "ws_status: {:?}", g.ws_status);
    let _ = writeln!(
        out,
        "user: {}",
        g.user.as_ref().map(|u| u.username.as_str()).unwrap_or("-")
    );
    if let Some(lobby) = &g.lobby {
        let _ = writeln!(
            out,
            "lobby: code={} status={:?} race_type={:?} game={} category={:?}",
            lobby.code, lobby.lobby_status, lobby.race_type, lobby.game_name, lobby.category_name
        );
    } else {
        let _ = writeln!(out, "lobby: none");
    }
    let _ = writeln!(out, "capture_source: {:?}", g.capture_source);
    let _ = writeln!(
        out,
        "preview: active={} starting={} gen={} last_jpeg_bytes={}",
        g.preview.is_some(),
        g.preview_starting,
        g.preview_gen,
        g.preview_last_jpeg.as_ref().map(|j| j.len()).unwrap_or(0)
    );
    let _ = writeln!(
        out,
        "stream: active={} finalizing={}",
        g.stream.is_some(),
        g.stream_finalizing
    );
    let _ = writeln!(out, "replay_base: {:?}", g.replay_base);
    let _ = writeln!(out);
}

fn write_environment(out: &mut String, app: &AppHandle) {
    let _ = writeln!(out, "=== Environment ===");
    match stream::ffmpeg_path() {
        Ok(p) => {
            let _ = writeln!(out, "ffmpeg: {}", p.display());
        }
        Err(e) => {
            let _ = writeln!(out, "ffmpeg: NOT FOUND ({e})");
        }
    }

    let settings = crate::settings::load_stream_settings(app);
    let _ = writeln!(
        out,
        "stream settings: bitrate={}kbps fps={} resolution={}p encoder_pref={}",
        settings.bitrate_kbps, settings.framerate, settings.resolution, settings.encoder
    );
    let _ = writeln!(
        out,
        "replay: dir={} autodelete={} casual={} delete_uploaded={}",
        settings.replay_dir,
        settings.replay_autodelete,
        settings.replay_casual,
        settings.replay_delete_uploaded
    );
    let _ = writeln!(
        out,
        "encoder detected: {}",
        stream::encoder::detected()
            .map(|e| e.name().to_string())
            .unwrap_or_else(|| "(not probed yet)".into())
    );

    #[cfg(windows)]
    {
        for bin in ["graphics-hook64.dll", "graphics-hook32.dll"] {
            match stream::gamecapture::resolve_binary(bin) {
                Ok(p) => {
                    let _ = writeln!(out, "game-capture binary {bin}: {}", p.display());
                }
                Err(e) => {
                    let _ = writeln!(out, "game-capture binary {bin}: MISSING ({e})");
                }
            }
        }
    }
    let _ = writeln!(out);
}

#[cfg(windows)]
fn write_capture(out: &mut String, app: &AppHandle, state: &SharedState) {
    use crate::stream::wgc::CaptureTarget;
    use crate::stream::CaptureSource;

    let _ = writeln!(out, "=== Capture diagnostics ===");
    let source = stream::current_source(app, state);
    match &source {
        CaptureSource::Window { hwnd, title } => {
            let _ = writeln!(out, "selected source: window \"{title}\" (hwnd={hwnd:#x})");
            let _ = writeln!(out, "owning process: {}", stream::gamecapture::describe(*hwnd));
            match stream::capture::target_size_even(CaptureTarget::Window { hwnd: *hwnd }) {
                Ok((w, h)) => {
                    let _ = writeln!(out, "capture size: {w}x{h}");
                }
                Err(e) => {
                    let _ = writeln!(out, "capture size: unavailable ({e})");
                }
            }
        }
        CaptureSource::Monitor { index } => {
            let _ = writeln!(out, "selected source: monitor {index}");
            match stream::list_monitors() {
                Ok(monitors) => {
                    for m in &monitors {
                        let _ = writeln!(
                            out,
                            "  monitor {}: {}x{} primary={} device={}",
                            m.index, m.width, m.height, m.primary, m.device_name
                        );
                    }
                }
                Err(e) => {
                    let _ = writeln!(out, "  monitor list unavailable: {e}");
                }
            }
            let path = if stream::capture::hmonitor_for_index(*index).is_some() {
                "WGC display capture"
            } else {
                "ddagrab (WGC unavailable for this monitor)"
            };
            let _ = writeln!(out, "expected path: {path}");
        }
    }
    let _ = writeln!(out);
}

#[cfg(not(windows))]
fn write_capture(out: &mut String, _app: &AppHandle, _state: &SharedState) {
    let _ = writeln!(out, "=== Capture diagnostics ===");
    let _ = writeln!(out, "(not supported on this platform)");
    let _ = writeln!(out);
}

fn write_log(out: &mut String) {
    let _ = writeln!(out, "=== Log ===");
    for line in logging::snapshot() {
        let _ = writeln!(out, "{line}");
    }
}
