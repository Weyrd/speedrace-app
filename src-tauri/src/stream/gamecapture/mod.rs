#[cfg(windows)]
mod capture;
#[cfg(windows)]
mod frame;
#[cfg(windows)]
mod inject;
mod offsets;
mod protocol;
#[cfg(windows)]
mod session;

#[cfg(windows)]
pub(crate) use capture::{start, GameCaptureHandle};

use std::path::PathBuf;

pub(crate) fn resolve_binary(name: &str) -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for cand in [
                dir.join("gamecapture").join(name),
                dir.join("resources").join("gamecapture").join(name),
            ] {
                if cand.exists() {
                    return Ok(cand);
                }
            }
        }
    }
    #[cfg(debug_assertions)]
    {
        let c = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join("gamecapture")
            .join(name);
        if c.exists() {
            return Ok(c);
        }
    }
    Err(format!(
        "game-capture binary '{name}' not found; run src-tauri/scripts/get-game-capture.ps1"
    ))
}
