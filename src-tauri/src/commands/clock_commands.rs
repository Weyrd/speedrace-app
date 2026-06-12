use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::config;
use crate::settings;
use crate::state::SharedState;

// Refresh at most once a day
const CLOCK_CACHE_TTL_MS: i64 = 24 * 60 * 60 * 1000;
const SAMPLE_COUNT: usize = 5;

#[derive(Serialize)]
pub struct ClockOffset {
    pub offset_ms: i64,
    pub synced_at: i64,
}

#[derive(Deserialize)]
struct TimeData {
    server_time_ms: i64,
}

#[derive(Deserialize)]
struct TimeEnvelope {
    data: TimeData,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// Keep the smallest round-trip sample: its midpoint estimate is most trustworthy
async fn measure_offset() -> Result<i64, String> {
    let http = reqwest::Client::new();
    let url = config::api_url("/api/v1/time");

    let mut best: Option<(i64, i64)> = None; // (rtt, offset)
    for _ in 0..SAMPLE_COUNT {
        let t0 = now_ms();
        let envelope = http
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .json::<TimeEnvelope>()
            .await
            .map_err(|e| e.to_string())?;
        let t1 = now_ms();

        let rtt = t1 - t0;
        let offset = envelope.data.server_time_ms - (t0 + t1) / 2;
        if best.map(|(b, _)| rtt < b).unwrap_or(true) {
            best = Some((rtt, offset));
        }
    }

    best.map(|(_, offset)| offset)
        .ok_or_else(|| "no clock samples".to_string())
}

// Mirrors the offset into SharedState so the hotkey-finish path stays fair.
#[tauri::command]
pub async fn sync_clock(
    app: AppHandle,
    state: State<'_, SharedState>,
    force: bool,
) -> Result<ClockOffset, String> {
    if !force {
        if let Some((offset, synced_at)) = settings::load_clock_offset(&app) {
            if now_ms() - synced_at < CLOCK_CACHE_TTL_MS {
                if let Ok(mut guard) = state.lock() {
                    guard.clock_offset_ms = offset;
                }
                return Ok(ClockOffset {
                    offset_ms: offset,
                    synced_at,
                });
            }
        }
    }

    let offset = measure_offset().await?;
    let synced_at = now_ms();
    settings::save_clock_offset(&app, offset, synced_at)?;
    if let Ok(mut guard) = state.lock() {
        guard.clock_offset_ms = offset;
    }
    Ok(ClockOffset {
        offset_ms: offset,
        synced_at,
    })
}
