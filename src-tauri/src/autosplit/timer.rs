use crate::autosplit::now_epoch_ms;
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::SharedState;
use livesplit_auto_splitting::{LogLevel, Timer, TimerState};
use std::fmt;
use tauri::AppHandle;

pub struct SpeedraceTimer {
    pub app: AppHandle,
    pub state: SharedState,
}

impl Timer for SpeedraceTimer {
    fn state(&self) -> TimerState {
        // running -> wasm can call timer.Reset() and check if run is active
        if self.state.lock().unwrap().run_active {
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
        let source = {
            let Ok(guard) = self.state.lock() else { return };
            guard.autosplit_source
        };
        match source {
            // Committed to WASM (gun passed): fire normally
            Some(crate::state::AutosplitSource::Wasm) => {
                crate::autosplit::split::fire_split(&self.app, &self.state)
            }
            // LiveSplit won the race: WASM ignore
            Some(crate::state::AutosplitSource::LiveSplit) => {}
            // Pre-gun crossing (source undecided): treat as an early start
            None => crate::autosplit::split::buffer_early_split(&self.app, &self.state),
        }
    }

    fn set_variable(&mut self, key: &str, value: &str) {
        let Ok(parsed): Result<i64, _> = value.parse() else {
            return;
        };

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        let counter_name = key.to_string();

        let post = {
            let mut guard = match self.state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(race_start_at) = guard.race_start_at else {
                return;
            };
            let at_ms = ((now_ms + guard.clock_offset_ms) - race_start_at).max(0) as u64;
            let Some(lobby) = guard.lobby.as_ref() else {
                return;
            };
            let lobby_id = lobby.lobby_id.clone();
            let sample = crate::counter::CounterSample {
                value: parsed,
                split_index: Some(guard.current_split_index),
                at_ms,
            };
            let cfg = guard
                .counter_config
                .as_ref()
                .and_then(|c| c.iter().find(|x| x.counter_name == counter_name).cloned());
            match crate::counter::resolve_action(cfg.as_ref()) {
                crate::counter::CounterAction::Drop => None,
                crate::counter::CounterAction::Buffer(mode) => {
                    guard
                        .counter_buffers
                        .entry(counter_name.clone())
                        .or_insert_with(|| crate::counter::CounterBuffer::for_mode(mode))
                        .record(sample);
                    None
                }
                crate::counter::CounterAction::Post => Some((lobby_id, sample)),
            }
        };

        let Some((lobby_id, sample)) = post else {
            return;
        };
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) =
                crate::api::lobby::post_player_counter(&app, &lobby_id, counter_name, vec![sample])
                    .await
            {
                mlog!(LogCat::Autosplit, "[autosplit] post_player_counter: {e}");
            }
        });
    }

    fn start(&mut self) {
        crate::autosplit::run_started::mark_run_start(&self.app, &self.state, now_epoch_ms());
    }
    fn skip_split(&mut self) {}
    fn undo_split(&mut self) {}
    fn reset(&mut self) {
        if let Ok(mut g) = self.state.lock() {
            crate::state::reset_run_start(&mut g);
        }
        let app = self.app.clone();
        let state = self.state.clone();
        tauri::async_runtime::spawn(async move {
            crate::ws::handler::report_autosplit_state_not_running(&app, &state).await;
        });
    }
    fn set_game_time(&mut self, t: livesplit_auto_splitting::time::Duration) {
        // Mid-run WASM get run_started_at_ms = now − IGT
        let igt = t.whole_milliseconds() as i64;
        let at = {
            let Ok(mut g) = self.state.lock() else { return };
            // A live run's IGT climbs a frozen menu IGT does not.
            let advancing = g.wasm_last_igt.is_some_and(|prev| igt > prev);
            g.wasm_last_igt = Some(igt);
            if g.run_start_instant.is_some() {
                return;
            }
            match g.app_state {
                AppState::StreamSetup | AppState::WaitingForStart => {
                    if igt <= 0 || !advancing {
                        return;
                    }
                }
                AppState::RaceInProgress => {
                    let gun_passed = g
                        .race_start_at
                        .is_some_and(|start| now_epoch_ms() + g.clock_offset_ms >= start);
                    if !gun_passed || igt <= 0 {
                        return;
                    }
                }
                _ => return,
            }
            now_epoch_ms() - igt
        };
        crate::autosplit::run_started::mark_run_start(&self.app, &self.state, at);
    }
    fn pause_game_time(&mut self) {}
    fn resume_game_time(&mut self) {}
    fn log_auto_splitter(&mut self, msg: fmt::Arguments) {
        mlog!(LogCat::Wasm, "[wasm] {msg}");
    }
    fn log_runtime(&mut self, msg: fmt::Arguments, _: LogLevel) {
        mlog!(LogCat::Wasm, "[wasm-rt] {msg}");
    }
}
