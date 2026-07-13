use super::WindowInfo;

#[cfg(windows)]
#[tauri::command]
pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_capture::window::Window;

    let wins = Window::enumerate().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for w in wins {
        let Ok(title) = w.title() else { continue };
        if title.trim().is_empty() {
            continue;
        }
        // too small
        if w.width().unwrap_or(0) < 200 || w.height().unwrap_or(0) < 150 {
            continue;
        }
        // DWM cloaked = invisible UWP/ghost windows
        let mut cloaked: u32 = 0;
        let hwnd = w.as_raw_hwnd();
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
        out.push(WindowInfo {
            hwnd: hwnd as u64,
            title,
            process_name: w.process_name().unwrap_or_default(),
        });
    }
    Ok(out)
}

#[cfg(not(windows))]
#[tauri::command]
pub fn list_windows() -> Result<Vec<WindowInfo>, String> {
    Err("window capture is only supported on Windows".into())
}
