use super::overlay_style::{DEFAULT_STYLE, FONT_FILE, MIN_FONT_PX};
use crate::logging::{mlog, LogCat};
use crate::state::{LockGlobalState, SharedState};
use std::path::{Path, PathBuf};

const TIMER_FILE: &str = "overlay_timer.txt";
const TICK_MS: u64 = 100;

fn row_file(r: usize) -> String {
    format!("overlay_row{r}.txt")
}

fn init_files(dir: &Path) -> std::io::Result<()> {
    let mut paths = vec![dir.join(TIMER_FILE)];
    paths.extend((0..DEFAULT_STYLE.max_splits).map(|r| dir.join(row_file(r))));
    for p in paths {
        if !p.exists() {
            std::fs::write(&p, b"")?;
        }
    }
    Ok(())
}

fn write_live(path: &Path, text: &str) {
    use std::io::Write;
    let Ok(mut f) = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
    else {
        return;
    };
    if f.write_all(text.as_bytes()).is_ok() {
        let _ = f.set_len(text.len() as u64);
    }
}

fn escape_path(p: &Path) -> Option<String> {
    let s = p.to_str()?.replace('\\', "/");
    if s.contains('\'') {
        return None;
    }
    Some(s.replace(':', "\\:"))
}

pub(super) fn filter_chain(dir: &Path, out_height: u32) -> Option<String> {
    let style = &DEFAULT_STYLE;
    if !drawtext_available() {
        mlog!(
            LogCat::Stream,
            "[overlay] disabled: sidecar has no drawtext"
        );
        return None;
    }
    let font = match resolve_font() {
        Ok(f) => escape_path(&f)?,
        Err(e) => {
            mlog!(LogCat::Stream, "[overlay] disabled: {e}");
            return None;
        }
    };
    if let Err(e) = init_files(dir) {
        mlog!(
            LogCat::Stream,
            "[overlay] disabled: textfile init failed: {e}"
        );
        return None;
    }
    let px = |frac: f32| ((frac * out_height as f32).round() as u32).max(1);
    let timer_fs = px(style.timer_font).max(MIN_FONT_PX);
    let split_fs = px(style.split_font).max(MIN_FONT_PX);
    let margin = px(style.margin);
    let pad = px(style.pad);
    let row_gap = px(style.row_gap);
    let timer_line = timer_fs * 6 / 5 + 2 * pad;
    let split_line = split_fs * 6 / 5 + 2 * pad;

    let x = |_fs: u32| format!("w-tw-{margin}");
    let y = |row_top: u32| format!("{}", margin + row_top);

    let chip = |file: &str, fs: u32, color: &str, alpha: f32, x: String, y: String| {
        format!(
            "drawtext=fontfile='{font}':textfile='{file}':reload=1:fontcolor={color}@{alpha}:fontsize={fs}:box=1:boxcolor={}@{}:boxborderw={pad}:x={x}:y={y}",
            style.card_color, style.card_alpha
        )
    };

    let timer_path = escape_path(&dir.join(TIMER_FILE))?;
    let mut parts = vec![chip(
        &timer_path,
        timer_fs,
        style.timer_color,
        1.0,
        x(timer_fs),
        y(0),
    )];
    for r in 0..style.max_splits {
        let p = escape_path(&dir.join(row_file(r)))?;
        let top = timer_line + row_gap + r as u32 * (split_line + row_gap);
        parts.push(chip(
            &p,
            split_fs,
            style.split_color,
            style.split_alpha,
            x(split_fs),
            y(top),
        ));
    }
    Some(parts.join(","))
}

fn drawtext_available() -> bool {
    static AVAIL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *AVAIL.get_or_init(|| {
        let Ok(ffmpeg) = super::ffmpeg_path() else {
            return false;
        };
        let mut cmd = std::process::Command::new(ffmpeg);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        cmd.args(["-hide_banner", "-filters"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(" drawtext "))
            .unwrap_or(false)
    })
}

fn resolve_font() -> Result<PathBuf, String> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for cand in [
                dir.join("fonts").join(FONT_FILE),
                dir.join("resources").join("fonts").join(FONT_FILE),
            ] {
                if cand.exists() {
                    return Ok(cand);
                }
            }
        }
    }
    #[cfg(debug_assertions)]
    {
        let c = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("fonts")
            .join(FONT_FILE);
        if c.exists() {
            return Ok(c);
        }
    }
    Err("overlay font not found".into())
}

fn fmt_timer(elapsed_ms: i64) -> String {
    let sign = if elapsed_ms < 0 { "-" } else { "" };
    let ms = elapsed_ms.unsigned_abs();
    let tenths = (ms % 1000) / 100;
    let secs = ms / 1000;
    format!(
        "{sign}{:02}:{:02}:{:02}.{tenths}",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

fn fmt_split_time(ms: u64) -> String {
    let tenths = (ms % 1000) / 100;
    let secs = ms / 1000;
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}.{tenths}")
    } else {
        format!("{m}:{s:02}.{tenths}")
    }
}

fn sanitize_name(name: &str, max: usize) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || " _.()-".contains(*c))
        .take(max)
        .collect()
}

pub(crate) fn record_split(state: &SharedState, name: &str, end_ms: u64) {
    let style = &DEFAULT_STYLE;
    let row = format!(
        "{:<width$} {:>10}",
        sanitize_name(name, style.name_max_chars),
        fmt_split_time(end_ms),
        width = style.name_max_chars
    );
    let (dir, rows) = {
        let mut g = state.lock_state();
        let Some(dir) = g
            .replay_base
            .as_ref()
            .and_then(|b| super::replay::parts_dir(b))
        else {
            return;
        };
        g.overlay_recent_splits.insert(0, row);
        g.overlay_recent_splits.truncate(style.max_splits);
        (dir, g.overlay_recent_splits.clone())
    };
    for r in 0..style.max_splits {
        write_live(&dir.join(row_file(r)), rows.get(r).map_or("", |s| s));
    }
}

pub(super) fn spawn_ticker(state: SharedState, mut stop_rx: tokio::sync::watch::Receiver<bool>) {
    tauri::async_runtime::spawn(async move {
        let mut last = String::new();
        loop {
            tokio::select! {
                _ = stop_rx.changed() => return,
                _ = tokio::time::sleep(std::time::Duration::from_millis(TICK_MS)) => {}
            }
            let (dir, elapsed) = {
                let g = state.lock_state();
                let Some(dir) = g
                    .replay_base
                    .as_ref()
                    .and_then(|b| super::replay::parts_dir(b))
                else {
                    continue;
                };
                let Some(rs) = g.race_start_at else { continue };
                (dir, g.server_now_ms() - rs)
            };
            let text = fmt_timer(elapsed);
            if text != last {
                write_live(&dir.join(TIMER_FILE), &text);
                last = text;
            }
        }
    });
}
