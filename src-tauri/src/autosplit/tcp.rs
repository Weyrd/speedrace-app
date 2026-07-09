use crate::autosplit::now_epoch_ms;
use crate::logging::{mlog, LogCat};
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
const PROBE_TIMEOUT_MS: u64 = 1000;

pub const RECONNECT_DELAY_MS: u64 = 1000;

pub async fn connect() -> Option<tokio::net::TcpStream> {
    let mut stream = match timeout(
        Duration::from_millis(CONNECT_TIMEOUT_MS),
        tokio::net::TcpStream::connect(LIVESPLIT_ADDR),
    )
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            mlog!(LogCat::LiveSplit, "[livesplit-tcp] not available: {e}");
            return None;
        }
        Err(_) => {
            mlog!(LogCat::LiveSplit, "[livesplit-tcp] connect timeout");
            return None;
        }
    };
    let _ = stream.set_nodelay(true);
    // A TCP handshake alone proves nothing: a WebSocket server (or anything else)
    // can squat on 16834. Require a valid getsplitindex reply before trusting it.
    if !probe_protocol(&mut stream).await {
        mlog!(
            LogCat::LiveSplit,
            "[livesplit-tcp] 16834 open but not LiveSplit Server protocol — ignoring"
        );
        return None;
    }
    mlog!(LogCat::LiveSplit, "[livesplit-tcp] connected");
    Some(stream)
}

async fn probe_protocol(stream: &mut tokio::net::TcpStream) -> bool {
    let (read_half, mut write_half) = stream.split();
    if write_half.write_all(b"getsplitindex\r\n").await.is_err() {
        return false;
    }
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();
    match timeout(
        Duration::from_millis(PROBE_TIMEOUT_MS),
        reader.read_line(&mut line),
    )
    .await
    {
        Ok(Ok(n)) if n > 0 => line.trim().parse::<i32>().is_ok(),
        _ => false,
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
    let mut name_checked_index: i32 = -1;
    let mut forced_start = false;
    let mut run_start_captured = false;
    let mut saw_not_running = false;
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
            mlog!(
                LogCat::LiveSplit,
                "[livesplit-tcp] WASM locked in as source — yielding"
            );
            break;
        }
        // Fire splits only when LiveSplit is the committed source; otherwise just track position.
        let fire = phase == crate::models::AppState::RaceInProgress
            && source == Some(crate::state::AutosplitSource::LiveSplit);

        if let Err(e) = writer.write_all(b"getsplitindex\r\n").await {
            mlog!(LogCat::LiveSplit, "[livesplit-tcp] write error: {e}");
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
                mlog!(
                    LogCat::LiveSplit,
                    "[livesplit-tcp] read timeout — reconnecting"
                );
                break;
            }
            Ok(Ok(0)) => {
                mlog!(
                    LogCat::LiveSplit,
                    "[livesplit-tcp] server closed connection"
                );
                break;
            }
            Ok(Err(e)) => {
                mlog!(LogCat::LiveSplit, "[livesplit-tcp] read error: {e}");
                break;
            }
            Ok(Ok(_)) => {}
        }

        let index = line.trim().parse::<i32>().unwrap_or(-1);

        if tick.is_multiple_of(100) {
            mlog!(
                LogCat::LiveSplit,
                "[livesplit-tcp] poll #{tick} index={index} last={last_index} fire={fire}"
            );
        }
        tick = tick.wrapping_add(1);

        // Capture the run start: a live NotRunning→Running edge is trusted; a first-seen running
        // timer is reconstructed from elapsed, or None if getcurrenttime fails.
        if index < 0 {
            saw_not_running = true;
            if run_start_captured {
                if let Ok(mut g) = state.lock() {
                    crate::state::reset_run_start(&mut g);
                }
                run_start_captured = false;
                crate::ws::handler::report_autosplit_state(&app, &state).await;
            }
        } else if index >= 0 && !run_start_captured {
            if saw_not_running {
                // Raw local instant; no clock_offset (back anchors run start to its own now).
                crate::autosplit::run_started::mark_run_start(&app, &state, now_epoch_ms());
                run_start_captured = true;
            } else {
                if writer.write_all(b"getcurrenttime\r\n").await.is_err() {
                    break;
                }
                line.clear();
                let time_res = timeout(
                    Duration::from_millis(READ_TIMEOUT_MS),
                    reader.read_line(&mut line),
                )
                .await;
                let elapsed = match time_res {
                    Ok(Ok(n)) if n > 0 => {
                        crate::autosplit::early_start::parse_livesplit_time_ms(line.trim())
                    }
                    _ => None,
                };
                // No getcurrenttime: can't anchor the start. The next split will forfeit.
                if let Some(elapsed) = elapsed {
                    // Reconstruct raw local instant; no clock_offset (back anchors to its now).
                    let at = crate::autosplit::early_start::run_start_from_elapsed(
                        now_epoch_ms(),
                        elapsed,
                    );
                    crate::autosplit::run_started::mark_run_start(&app, &state, at);
                }
                run_start_captured = true;
            }
        }

        // If LiveSplit is our source but the runner's timer never started (e.g. their
        // auto-start is bound to a different game/level than the one being raced), force
        // it once so getcurrentsplitname becomes readable and we can verify the splits.
        // starttimer is a no-op if already running and sends no reply, so don't read one.
        if fire && index < 0 && !forced_start {
            mlog!(
                LogCat::LiveSplit,
                "[livesplit-tcp] timer NotRunning at race time — sending starttimer"
            );
            if let Err(e) = writer.write_all(b"starttimer\r\n").await {
                mlog!(
                    LogCat::LiveSplit,
                    "[livesplit-tcp] starttimer write error: {e}"
                );
                break;
            }
            forced_start = true;
        }

        if !fire {
            last_index = index;
        } else if last_index < 0 && index >= 0 {
            // Timer just started: catch up completed splits. The burst can't recover intermediate
            // checkpoint times, so skip them and record only the final at real now.
            if index > 0 {
                mlog!(
                    LogCat::LiveSplit,
                    "[livesplit-tcp] catching up {index} split(s) (index jumped to {index})"
                );
                for _ in 0..(index - 1) {
                    crate::autosplit::split::skip_split(&app, &state);
                }
                crate::autosplit::split::fire_split(&app, &state);
            }
            last_index = index;
        } else if index > last_index {
            let steps = (index - last_index) as usize;
            mlog!(
                LogCat::LiveSplit,
                "[livesplit-tcp] split {last_index} → {index} ({steps} split(s))"
            );
            for _ in 0..(steps - 1) {
                crate::autosplit::split::skip_split(&app, &state);
            }
            crate::autosplit::split::fire_split(&app, &state);
            last_index = index;
        } else if index < last_index {
            // Runner reset their timer
            mlog!(
                LogCat::LiveSplit,
                "[livesplit-tcp] index reset {last_index} → {index}, re-arming"
            );
            last_index = index;
        }

        // Once per split, compare the runner's current split name to our expected
        // segment so we can flag (and refuse to record) a different split set.
        if index >= 0 && index != name_checked_index {
            let expected = {
                let g = state.lock().unwrap();
                g.split_run.as_ref().and_then(|r| {
                    let i = index as usize;
                    (i < r.len()).then(|| r.segment(i).name().to_string())
                })
            };
            match expected {
                Some(expected) => {
                    if writer.write_all(b"getcurrentsplitname\r\n").await.is_err() {
                        break;
                    }
                    line.clear();
                    let name_res = timeout(
                        Duration::from_millis(READ_TIMEOUT_MS),
                        reader.read_line(&mut line),
                    )
                    .await;
                    if let Ok(Ok(n)) = name_res {
                        let actual = line.trim();
                        // "-" means LiveSplit has no current split yet; recheck later.
                        if n > 0 && actual != "-" {
                            let matches = actual.eq_ignore_ascii_case(expected.trim());
                            if !matches {
                                mlog!(LogCat::LiveSplit,
                                    "[livesplit-tcp] split name mismatch at {index}: livesplit='{actual}' expected='{expected}'"
                                );
                            }
                            let changed = {
                                let mut g = state.lock().unwrap();
                                let prev = g.livesplit_splits_match;
                                g.livesplit_splits_match = Some(matches);
                                prev != Some(matches)
                            };
                            if changed {
                                crate::ws::handler::report_autosplit_state(&app, &state).await;
                            }
                            name_checked_index = index;
                        }
                    }
                }
                None => name_checked_index = index,
            }
        }

        sleep(Duration::from_millis(POLL_MS)).await;
    }

    mlog!(LogCat::LiveSplit, "[livesplit-tcp] poll stopped");
}
