use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{Duration, sleep, timeout};
use tauri::AppHandle;
use crate::state::SharedState;

const LIVESPLIT_ADDR: &str = "127.0.0.1:16834";
const POLL_MS: u64 = 100;
const READ_TIMEOUT_MS: u64 = 3000;
const CONNECT_TIMEOUT_MS: u64 = 500;

/// Try to connect to LiveSplit TCP server. If successful, spawn the poll loop and return true.
/// Returns false immediately (within 500ms) if LiveSplit is not running.
pub async fn start(
    app: AppHandle,
    state: SharedState,
    cancel: Arc<AtomicBool>,
) -> bool {
    let stream = match timeout(
        Duration::from_millis(CONNECT_TIMEOUT_MS),
        tokio::net::TcpStream::connect(LIVESPLIT_ADDR),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            eprintln!("[livesplit-tcp] not available: {e}");
            return false;
        }
        Err(_) => {
            eprintln!("[livesplit-tcp] connect timeout");
            return false;
        }
    };

    let _ = stream.set_nodelay(true);
    eprintln!("[livesplit-tcp] connected");

    tauri::async_runtime::spawn(async move {
        poll_loop(stream, app, state, cancel).await;
    });
    true
}

async fn poll_loop(
    stream: tokio::net::TcpStream,
    app: AppHandle,
    state: SharedState,
    cancel: Arc<AtomicBool>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();
    let mut last_index: i32 = -1;
    let mut tick: u32 = 0;

    loop {
        if cancel.load(Ordering::SeqCst) {
            break;
        }

        if let Err(e) = writer.write_all(b"getsplitindex\r\n").await {
            eprintln!("[livesplit-tcp] write error: {e}");
            break;
        }

        line.clear();
        let read_result = timeout(
            Duration::from_millis(READ_TIMEOUT_MS),
            reader.read_line(&mut line),
        )
        .await;

        match read_result {
            Err(_) => {
                eprintln!("[livesplit-tcp] read timeout — LiveSplit.Server not responding");
                break;
            }
            Ok(Ok(0)) => {
                eprintln!("[livesplit-tcp] server closed connection");
                break;
            }
            Ok(Err(e)) => {
                eprintln!("[livesplit-tcp] read error: {e}");
                break;
            }
            Ok(Ok(_)) => {}
        }

        let index = line.trim().parse::<i32>().unwrap_or(-1);

        if tick % 100 == 0 {
            eprintln!("[livesplit-tcp] poll #{tick} index={index} last={last_index}");
        }
        tick = tick.wrapping_add(1);

        if last_index < 0 && index < 0 {
            // Timer not yet started
        } else if last_index < 0 && index >= 0 {
            // First positive reading: catch up on splits already completed before we connected
            let catch_up = index as usize;
            if catch_up > 0 {
                eprintln!("[livesplit-tcp] catching up {catch_up} split(s) (index jumped to {index})");
                for _ in 0..catch_up {
                    crate::autosplit::split::fire_split(&app, &state);
                }
            }
            last_index = index;
        } else if last_index >= 0 && index < 0 {
            // Timer reset to -1 after race was running — race finished or manually reset
            eprintln!("[livesplit-tcp] index reset to -1 after last={last_index}, stopping");
            break;
        } else if index > last_index {
            let steps = (index - last_index) as usize;
            eprintln!("[livesplit-tcp] split {last_index} → {index} ({steps} split(s))");
            for _ in 0..steps {
                crate::autosplit::split::fire_split(&app, &state);
            }
            last_index = index;
        } else if index >= 0 && index < last_index {
            eprintln!("[livesplit-tcp] index reset {last_index} → {index}");
            last_index = index;
        }

        sleep(Duration::from_millis(POLL_MS)).await;
    }

    eprintln!("[livesplit-tcp] stopped");
}
