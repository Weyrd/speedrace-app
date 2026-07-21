use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

use super::StopFlag;
use crate::logging::{mlog, LogCat};

const PRIME_DEADLINE: Duration = Duration::from_secs(2);

pub(crate) fn new_video_pipe() -> Result<(String, NamedPipeServer), String> {
    let name = format!(r"\\.\pipe\momentum_video_{:016x}", rand::random::<u64>());
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .map_err(|e| format!("video pipe create failed: {e}"))?;
    Ok((name, server))
}

pub(crate) fn spawn_paced_writer(
    server: NamedPipeServer,
    latest: Arc<Mutex<Vec<u8>>>,
    stop: StopFlag,
    primed: StopFlag,
    fps: u32,
    frame_bytes: usize,
) -> tauri::async_runtime::JoinHandle<()> {
    let period = Duration::from_millis((1000 / fps.max(1)).max(1) as u64);
    tauri::async_runtime::spawn(async move {
        // wait for ffmpeg to open the read end
        if server.connect().await.is_err() {
            return;
        }
        let deadline = tokio::time::Instant::now() + PRIME_DEADLINE;
        while !primed.load(Ordering::SeqCst) {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            if tokio::time::Instant::now() >= deadline {
                mlog!(
                    LogCat::Stream,
                    "[capture] no frame within 2s; starting black"
                );
                break;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
        let mut server = server;
        let mut ticker = tokio::time::interval(period);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut out = vec![0u8; frame_bytes];
        loop {
            ticker.tick().await;
            if stop.load(Ordering::SeqCst) {
                return; // dropping EOFs ffmpeg's video input
            }
            if let Ok(l) = latest.lock() {
                if l.len() == frame_bytes {
                    out.copy_from_slice(&l);
                }
            }
            if server.write_all(&out).await.is_err() {
                return;
            }
        }
    })
}
