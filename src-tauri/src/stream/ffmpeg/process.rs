use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, Command};

pub fn resolve_ffmpeg_path() -> Result<PathBuf, String> {
    let exe_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let c = dir.join(exe_name);
            if c.exists() {
                return Ok(c);
            }
        }
    }
    #[cfg(debug_assertions)]
    {
        let bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries");
        if let Ok(rd) = std::fs::read_dir(&bin) {
            for e in rd.flatten() {
                let n = e.file_name();
                let n = n.to_string_lossy();
                if n.starts_with("ffmpeg") && n.ends_with(".exe") {
                    return Ok(e.path());
                }
            }
        }
    }
    Err("ffmpeg sidecar not found; run src-tauri/scripts/get-ffmpeg.ps1".into())
}

pub(crate) async fn graceful_stop(child: &mut Child, stdin: &mut Option<tokio::process::ChildStdin>) {
    if let Some(si) = stdin.as_mut() {
        let _ = si.write_all(b"q\n").await;
        let _ = si.flush().await;
    }
    *stdin = None;
    if tokio::time::timeout(Duration::from_secs(3), child.wait())
        .await
        .is_err()
    {
        let _ = child.kill().await;
    }
}

#[cfg(windows)]
pub(crate) const NULL_SINK: &str = "NUL";
#[cfg(not(windows))]
pub(crate) const NULL_SINK: &str = "/dev/null";

pub(crate) fn ffmpeg_command(path: &Path) -> Command {
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000);
    cmd
}

pub(crate) fn spawn_ffmpeg(path: &Path, args: &[String]) -> Result<Child, String> {
    let mut cmd = ffmpeg_command(path);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = cmd.spawn().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    assign_to_job(&child);
    Ok(child)
}

#[cfg(windows)]
fn assign_to_job(child: &Child) {
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    static JOB: OnceLock<isize> = OnceLock::new();
    let handle = *JOB.get_or_init(|| unsafe {
        let Ok(h) = CreateJobObjectW(None, windows::core::PCWSTR::null()) else {
            return 0;
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let _ = SetInformationJobObject(
            h,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        h.0 as isize
    });

    if handle == 0 {
        return;
    }
    if let Some(raw) = child.raw_handle() {
        unsafe {
            let _ = AssignProcessToJobObject(HANDLE(handle as *mut _), HANDLE(raw as *mut _));
        }
    }
}
