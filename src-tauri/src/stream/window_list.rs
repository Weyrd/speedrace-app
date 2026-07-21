use super::WindowInfo;

#[cfg(windows)]
#[tauri::command]
pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    use crate::logging::{mlog, LogCat};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::UI::WindowsAndMessaging::IsIconic;
    use windows_capture::window::Window;

    let wins = Window::enumerate().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for w in wins {
        let Ok(title) = w.title() else { continue };
        if title.trim().is_empty() {
            continue;
        }
        let hwnd = w.as_raw_hwnd();
        let iconic = unsafe { IsIconic(HWND(hwnd)).as_bool() };
        let (width, height) = (w.width().unwrap_or(0), w.height().unwrap_or(0));
        if !iconic && (width < 200 || height < 150) {
            continue;
        }
        // DWM cloaked = invisible UWP/ghost windows
        let mut cloaked: u32 = 0;
        unsafe {
            let _ = DwmGetWindowAttribute(
                HWND(hwnd),
                DWMWA_CLOAKED,
                (&mut cloaked as *mut u32).cast(),
                std::mem::size_of::<u32>() as u32,
            );
        }
        if cloaked != 0 {
            continue;
        }
        if iconic {
            mlog!(LogCat::Stream, "[windows] included minimized '{title}'");
        }
        out.push(WindowInfo {
            hwnd: hwnd as u64,
            title,
            process_name: w.process_name().unwrap_or_default(),
            iconic,
        });
    }
    mlog!(LogCat::Stream, "[windows] listed {} window(s)", out.len());
    Ok(out)
}

#[cfg(not(windows))]
#[tauri::command]
pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    Err("window capture is only supported on Windows".into())
}

#[cfg(windows)]
pub(crate) fn game_window_for_pid(pid: u32) -> Option<(u64, String)> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct FindCtx {
        pid: u32,
        best: HWND,
        best_area: i64,
    }

    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let ctx = &mut *(lparam.0 as *mut FindCtx);
        let mut wpid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut wpid as *mut u32));
        if wpid == ctx.pid && IsWindowVisible(hwnd).as_bool() {
            let mut r = RECT::default();
            if GetWindowRect(hwnd, &mut r).is_ok() {
                let area = (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
                if area > ctx.best_area {
                    ctx.best_area = area;
                    ctx.best = hwnd;
                }
            }
        }
        BOOL(1)
    }

    let mut ctx = FindCtx {
        pid,
        best: HWND::default(),
        best_area: 0,
    };
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut ctx as *mut _ as isize));
        if ctx.best.0.is_null() {
            return None;
        }
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(ctx.best, &mut buf);
        let title = String::from_utf16_lossy(&buf[..n as usize]);
        Some((ctx.best.0 as u64, title))
    }
}
