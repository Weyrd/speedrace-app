use crate::models::AppState;
use crate::state::SharedState;
use livesplit_auto_splitting::{LogLevel, Timer, TimerState};
use std::fmt;
use tauri::AppHandle;

pub struct MomentumTimer {
    pub app: AppHandle,
    pub state: SharedState,
}

impl Timer for MomentumTimer {
    fn state(&self) -> TimerState {
        let guard = self.state.lock().unwrap();
        if guard.app_state == AppState::RaceInProgress {
            TimerState::Running
        } else {
            TimerState::NotRunning
        }
    }

    fn current_split_index(&self) -> Option<usize> {
        let guard = self.state.lock().unwrap();
        Some(guard.current_split_index as usize)
    }

    fn segment_splitted(&self, idx: usize) -> Option<bool> {
        let guard = self.state.lock().unwrap();
        Some((guard.current_split_index as usize) > idx)
    }

    fn split(&mut self) {
        // Only fire when WASM is the committed source
        {
            let Ok(guard) = self.state.lock() else { return };
            if guard.autosplit_source != Some(crate::state::AutosplitSource::Wasm) {
                return;
            }
        }
        crate::autosplit::split::fire_split(&self.app, &self.state);
    }

    fn set_variable(&mut self, key: &str, value: &str) {
        let Ok(parsed): Result<i64, _> = value.parse() else {
            return;
        };

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let (lobby_id, split_index, timestamp_ms) = {
            let guard = match self.state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(race_start_at) = guard.race_start_at else {
                return;
            };
            let timestamp_ms = ((now_ms + guard.clock_offset_ms) - race_start_at).max(0) as u64;
            let Some(lobby) = guard.lobby.as_ref() else {
                return;
            };
            (
                lobby.lobby_id.clone(),
                guard.current_split_index,
                timestamp_ms,
            )
        };

        let app = self.app.clone();
        let counter_name = key.to_string();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::api::lobby::post_player_counter(
                &app,
                &lobby_id,
                counter_name,
                parsed,
                Some(split_index),
                timestamp_ms,
            )
            .await
            {
                eprintln!("[autosplit] post_player_counter: {e}");
            }
        });
    }

    // Race clock is authoritative; no-ops for timer control methods
    fn start(&mut self) {}
    fn skip_split(&mut self) {}
    fn undo_split(&mut self) {}
    fn reset(&mut self) {}
    fn set_game_time(&mut self, _: livesplit_auto_splitting::time::Duration) {}
    fn pause_game_time(&mut self) {}
    fn resume_game_time(&mut self) {}
    fn log_auto_splitter(&mut self, msg: fmt::Arguments) {
        eprintln!("[wasm] {msg}");
    }
    fn log_runtime(&mut self, msg: fmt::Arguments, _: LogLevel) {
        eprintln!("[wasm-rt] {msg}");
    }
}
