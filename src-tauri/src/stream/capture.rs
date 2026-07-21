#[cfg(windows)]
use crate::logging::{mlog, LogCat};

#[cfg(windows)]
const WGC_FULLSCREEN_PROBE: std::time::Duration = std::time::Duration::from_millis(1500); // after try inject obs dll

#[cfg(windows)]
pub async fn start_capture_for(
    source: &super::CaptureSource,
    fps: u32,
) -> Result<Option<super::CaptureHandle>, String> {
    use super::wgc::{start_capture, CaptureTarget};
    use super::CaptureHandle;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsIconic;

    match source {
        super::CaptureSource::Window { hwnd, .. } => {
            let h = HWND(*hwnd as *mut std::ffi::c_void);
            let fullscreen = unsafe { IsIconic(h).as_bool() } || covers_monitor(h);
            if !fullscreen {
                let target = CaptureTarget::Window { hwnd: *hwnd };
                let (w, h2) = target_size_even(target)?;
                return Ok(Some(CaptureHandle::Wgc(start_capture(target, w, h2, fps)?)));
            }
            Ok(Some(capture_fullscreen_window(*hwnd, fps).await?))
        }
        super::CaptureSource::Monitor { index } => match hmonitor_for_index(*index) {
            Some(hmonitor) => {
                let target = CaptureTarget::Monitor { hmonitor };
                match target_size_even(target).and_then(|(w, h)| start_capture(target, w, h, fps)) {
                    Ok(handle) => Ok(Some(CaptureHandle::Wgc(handle))),
                    Err(e) => {
                        mlog!(
                            LogCat::Stream,
                            "[capture] wgc monitor capture failed ({e}); falling back to ddagrab"
                        );
                        Ok(None)
                    }
                }
            }
            None => {
                mlog!(
                    LogCat::Stream,
                    "[capture] monitor {index} not mapped; falling back to ddagrab"
                );
                Ok(None)
            }
        },
    }
}

#[cfg(windows)]
async fn capture_fullscreen_window(hwnd: u64, fps: u32) -> Result<super::CaptureHandle, String> {
    use super::wgc::{start_capture, CaptureTarget};
    use super::CaptureHandle;

    let target = CaptureTarget::Window { hwnd };
    if let Ok((w, h)) = target_size_even(target) {
        if let Ok(wgc) = start_capture(target, w, h, fps) {
            let deadline = tokio::time::Instant::now() + WGC_FULLSCREEN_PROBE;
            while tokio::time::Instant::now() < deadline {
                if wgc.primed.load(std::sync::atomic::Ordering::SeqCst) {
                    mlog!(
                        LogCat::Stream,
                        "[capture] window {hwnd:#x} fullscreen but WGC composites (borderless); using WGC"
                    );
                    return Ok(CaptureHandle::Wgc(wgc));
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            mlog!(
                LogCat::Stream,
                "[capture] window {hwnd:#x} exclusive fullscreen (WGC black); injecting game-capture"
            );
            wgc.shutdown().await;
        }
    }

    let (gw, gh) = monitor_size_even(hwnd);
    let gc = super::gamecapture::start(hwnd, gw, gh, fps)
        .await
        .map_err(|e| format!("couldn't capture the game window: {e}"))?;
    Ok(CaptureHandle::Game(gc))
}

#[cfg(not(windows))]
pub async fn start_capture_for(
    _source: &super::CaptureSource,
    _fps: u32,
) -> Result<Option<super::CaptureHandle>, String> {
    Ok(None)
}

#[cfg(windows)]
fn even(v: i32) -> u32 {
    (v.max(2) as u32) & !1
}

#[cfg(windows)]
fn hmonitor_for_index(index: u32) -> Option<isize> {
    let device = super::list_monitors()
        .ok()?
        .into_iter()
        .find(|m| m.index == index)?
        .device_name;
    windows_capture::monitor::Monitor::enumerate()
        .ok()?
        .into_iter()
        .find(|m| m.device_name().map(|d| d == device).unwrap_or(false))
        .map(|m| m.as_raw_hmonitor() as isize)
}

#[cfg(windows)]
fn hmonitor_size(hmonitor: isize) -> Option<(i32, i32)> {
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(HMONITOR(hmonitor as *mut std::ffi::c_void), &mut info) }.as_bool()
    {
        let r = info.rcMonitor;
        Some((r.right - r.left, r.bottom - r.top))
    } else {
        None
    }
}

#[cfg(windows)]
fn monitor_size(hwnd: windows::Win32::Foundation::HWND) -> Option<(i32, i32)> {
    use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
    let mon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    hmonitor_size(mon.0 as isize)
}

#[cfg(windows)]
pub(crate) fn monitor_size_even(hwnd: u64) -> (u32, u32) {
    use windows::Win32::Foundation::HWND;
    monitor_size(HWND(hwnd as *mut std::ffi::c_void))
        .map(|(w, h)| (even(w), even(h)))
        .unwrap_or((1920, 1080))
}

#[cfg(windows)]
fn covers_monitor(hwnd: windows::Win32::Foundation::HWND) -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

    let mut wr = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut wr) }.is_err() {
        return false;
    }
    let mon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut mi = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if !unsafe { GetMonitorInfoW(mon, &mut mi) }.as_bool() {
        return false;
    }
    let mr = mi.rcMonitor;
    let tol = 2;
    (wr.left - mr.left).abs() <= tol
        && (wr.top - mr.top).abs() <= tol
        && (wr.right - mr.right).abs() <= tol
        && (wr.bottom - mr.bottom).abs() <= tol
}

#[cfg(windows)]
fn target_size_even(target: super::wgc::CaptureTarget) -> Result<(u32, u32), String> {
    use super::wgc::CaptureTarget;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::IsIconic;

    let (w, h) = match target {
        CaptureTarget::Monitor { hmonitor } => {
            hmonitor_size(hmonitor).ok_or_else(|| "monitor size unavailable".to_string())?
        }
        CaptureTarget::Window { hwnd } => {
            let hwin = HWND(hwnd as *mut std::ffi::c_void);
            if unsafe { IsIconic(hwin).as_bool() } {
                if let Some(s) = monitor_size(hwin) {
                    (s.0, s.1)
                } else {
                    window_size(hwnd)?
                }
            } else {
                window_size(hwnd)?
            }
        }
    };
    Ok((even(w), even(h)))
}

#[cfg(windows)]
fn window_size(hwnd: u64) -> Result<(i32, i32), String> {
    let window = windows_capture::window::Window::from_raw_hwnd(hwnd as *mut std::ffi::c_void);
    Ok((
        window.width().map_err(|e| e.to_string())?,
        window.height().map_err(|e| e.to_string())?,
    ))
}
