use crate::state::SharedState;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::{sleep, timeout, Duration};

const LIVESPLIT_ADDR: &str = "127.0.0.1:16834";
const POLL_MS: u64 = 100;
const READ_TIMEOUT_MS: u64 = 3000;
const CONNECT_TIMEOUT_MS: u64 = 500;

pub const RECONNECT_DELAY_MS: u64 = 1000;

pub async fn connect() -> Option<tokio::net::TcpStream> {
    match timeout(
        Duration::from_millis(CONNECT_TIMEOUT_MS),
        tokio::net::TcpStream::connect(LIVESPLIT_ADDR),
    )
    .await
    {
        Ok(Ok(s)) => {
            let _ = s.set_nodelay(true);
            eprintln!("[livesplit-tcp] connected");
            Some(s)
        }
        Ok(Err(e)) => {
            eprintln!("[livesplit-tcp] not available: {e}");
            None
        }
        Err(_) => {
            eprintln!("[livesplit-tcp] connect timeout");
            None
        }
    }
}

pub async fn poll_loop(
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

        let phase = state.lock().unwrap().app_state.clone();
        if !matches!(
            phase,
            crate::models::AppState::StreamSetup
                | crate::models::AppState::WaitingForStart
                | crate::models::AppState::RaceInProgress
        ) {
            break;
        }

        crate::ws::handler::maybe_commit_source(&state);
        let source = state.lock().unwrap().autosplit_source;
        if source == Some(crate::state::AutosplitSource::Wasm) {
            eprintln!("[livesplit-tcp] WASM locked in as source — yielding");
            break;
        }
        // Fire splits only when LiveSplit is the committed source; otherwise just track position.
        let fire = phase == crate::models::AppState::RaceInProgress
            && source == Some(crate::state::AutosplitSource::LiveSplit);

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
                eprintln!("[livesplit-tcp] read timeout — reconnecting");
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
            eprintln!("[livesplit-tcp] poll #{tick} index={index} last={last_index} fire={fire}");
        }
        tick = tick.wrapping_add(1);

        if !fire {
            last_index = index;
        } else if last_index < 0 && index >= 0 {
            // Timer just started catch up already-completed splits
            if index > 0 {
                eprintln!("[livesplit-tcp] catching up {index} split(s) (index jumped to {index})");
            }
            for _ in 0..index {
                crate::autosplit::split::fire_split(&app, &state);
            }
            last_index = index;
        } else if index > last_index {
            let steps = (index - last_index) as usize;
            eprintln!("[livesplit-tcp] split {last_index} → {index} ({steps} split(s))");
            for _ in 0..steps {
                crate::autosplit::split::fire_split(&app, &state);
            }
            last_index = index;
        } else if index < last_index {
            // Runner reset their timer
            eprintln!("[livesplit-tcp] index reset {last_index} → {index}, re-arming");
            last_index = index;
        }

        sleep(Duration::from_millis(POLL_MS)).await;
    }

    eprintln!("[livesplit-tcp] poll stopped");
}
