use super::protocol::GraphicsOffsets;

#[cfg(windows)]
pub(crate) fn load(process_is_64bit: bool) -> Result<GraphicsOffsets, String> {
    use std::sync::OnceLock;
    static C64: OnceLock<GraphicsOffsets> = OnceLock::new();
    static C32: OnceLock<GraphicsOffsets> = OnceLock::new();
    let cell = if process_is_64bit { &C64 } else { &C32 };
    if let Some(o) = cell.get() {
        return Ok(*o);
    }
    let o = probe(process_is_64bit)?;
    let _ = cell.set(o);
    Ok(*cell.get().unwrap_or(&o))
}

#[cfg(windows)]
fn probe(is64: bool) -> Result<GraphicsOffsets, String> {
    use std::os::windows::process::CommandExt;
    let name = if is64 {
        "get-graphics-offsets64.exe"
    } else {
        "get-graphics-offsets32.exe"
    };
    let exe = super::resolve_binary(name)?;
    let out = std::process::Command::new(&exe)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .output()
        .map_err(|e| format!("run {name}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{name} exited {:?}", out.status.code()));
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

#[cfg(not(windows))]
pub(crate) fn load(_process_is_64bit: bool) -> Result<GraphicsOffsets, String> {
    Err("game capture is only supported on Windows".into())
}

fn parse(text: &str) -> Result<GraphicsOffsets, String> {
    let mut o = GraphicsOffsets::default();
    let mut section = "";
    for line in text.lines() {
        let line = line.trim();
        if let Some(inner) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = inner;
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let val = parse_hex(v.trim())?;
        match (section, k.trim()) {
            ("d3d8", "present") => o.d3d8.present = val,
            ("d3d9", "present") => o.d3d9.present = val,
            ("d3d9", "present_ex") => o.d3d9.present_ex = val,
            ("d3d9", "present_swap") => o.d3d9.present_swap = val,
            ("d3d9", "d3d9_clsoff") => o.d3d9.d3d9_clsoff = val,
            ("d3d9", "is_d3d9ex_clsoff") => o.d3d9.is_d3d9ex_clsoff = val,
            ("dxgi", "present") => o.dxgi.present = val,
            ("dxgi", "present1") => o.dxgi.present1 = val,
            ("dxgi", "resize") => o.dxgi.resize = val,
            ("dxgi", "release") => o.dxgi2.release = val,
            _ => {}
        }
    }

    if o.dxgi.present == 0 {
        return Err("offsets probe returned no dxgi.present".into());
    }
    Ok(o)
}

fn parse_hex(s: &str) -> Result<u32, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u32::from_str_radix(s, 16).map_err(|e| format!("bad hex '{s}': {e}"))
}
