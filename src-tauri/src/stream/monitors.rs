use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MonitorInfo {
    pub index: u32,
    pub width: u32,
    pub height: u32,
    pub primary: bool,
    pub device_name: String,
}

// Outputs of DXGI adapter 0, in EnumOutputs order
#[cfg(windows)]
#[tauri::command]
pub fn list_monitors() -> Result<Vec<MonitorInfo>, String> {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, DXGI_ERROR_NOT_FOUND,
    };

    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| e.to_string())?;

        let mut monitors = Vec::new();
        let mut adapter_idx = 0u32;
        loop {
            let adapter: IDXGIAdapter1 = match factory.EnumAdapters1(adapter_idx) {
                Ok(a) => a,
                Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(e) => return Err(e.to_string()),
            };

            let mut out_idx = 0u32;
            loop {
                let output: IDXGIOutput = match adapter.EnumOutputs(out_idx) {
                    Ok(o) => o,
                    Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
                    Err(e) => return Err(e.to_string()),
                };
                let desc = output.GetDesc().map_err(|e| e.to_string())?;

                let r = desc.DesktopCoordinates;
                let width = (r.right - r.left).max(0) as u32;
                let height = (r.bottom - r.top).max(0) as u32;
                let device_name = String::from_utf16_lossy(&desc.DeviceName)
                    .trim_end_matches('\0')
                    .to_string();

                monitors.push(MonitorInfo {
                    index: out_idx,
                    width,
                    height,
                    primary: r.left == 0 && r.top == 0,
                    device_name,
                });
                out_idx += 1;
            }

            if !monitors.is_empty() {
                break;
            }
            adapter_idx += 1;
        }

        Ok(monitors)
    }
}

#[cfg(not(windows))]
#[tauri::command]
pub fn list_monitors() -> Result<Vec<MonitorInfo>, String> {
    Err("monitor capture is only supported on Windows".into())
}
