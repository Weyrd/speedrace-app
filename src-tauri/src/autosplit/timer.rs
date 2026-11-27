use crate::autosplit::now_epoch_ms;
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::{LockGlobalState, SharedState};
use livesplit_auto_splitting::{LogLevel, Timer, TimerState};
use std::fmt;
use tauri::AppHandle;

pub struct SpeedraceTimer {
    pub app: AppHandle,
    pub state: SharedState,
}

impl Timer for SpeedraceTimer {
    fn state(&self) -> TimerState {
        if self.state.lock_state().run_active {
            TimerState::Running
        } else {
            TimerState::NotRunning
        }
    }

    fn current_split_index(&self) -> Option<usize> {
        let guard = self.state.lock_state();
        Some(guard.current_split_index as usize)
    }

    fn segment_splitted(&self, idx: usize) -> Option<bool> {
        let guard = self.state.lock_state();
        Some((guard.current_split_index as usize) > idx)
    }

    fn split(&mut self) {
        let source = self.state.lock_state().autosplit.source;
        match source {
            Some(crate::state::AutosplitSource::Wasm) => {
                crate::autosplit::split::fire_split(&self.app, &self.state)
            }
            Some(crate::state::AutosplitSource::LiveSplit) => {}
            None => crate::autosplit::split::buffer_early_split(&self.app, &self.state),
        }
    }

    fn set_variable(&mut self, key: &str, value: &str) {
        let Ok(parsed): Result<i64, _> = value.parse() else {
            return;
        };

        if !crate::counter::is_valid_counter_name(key) {
            mlog!(
                LogCat::Counter,
                "[counter] name {key:?} is not plain ASCII (A-Z a-z 0-9 space _ -, 64 max), dropped"
            );
            return;
        }

        let post = crate::counter::admit_sample(&mut self.state.lock_state(), key, parsed);
        if let Some(post) = post {
            crate::counter::spawn_instant_post(&self.app, &self.state, post);
        }
    }

    fn start(&mut self) {
        crate::autosplit::run_started::mark_run_start(&self.app, &self.state, now_epoch_ms());
    }
    fn skip_split(&mut self) {}
    fn undo_split(&mut self) {
        let mut g = self.state.lock_state();
        if g.current_split_index == 0 {
            return;
        }
        g.current_split_index -= 1;
        mlog!(
            LogCat::Autosplit,
            "[wasm] undo_split: local index now {}, the split already reported to the back cannot be retracted",
            g.current_split_index
        );
    }
    fn reset(&mut self) {
        crate::state::reset_run_start(&mut self.state.lock_state());
        let app = self.app.clone();
        let state = self.state.clone();
        tauri::async_runtime::spawn(async move {
            crate::ws::handler::report_autosplit_state_not_running(&app, &state).await;
        });
    }
    fn set_game_time(&mut self, t: livesplit_auto_splitting::time::Duration) {
        let igt = t.whole_milliseconds() as i64;
        let at = {
            let mut g = self.state.lock_state();
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
                        .is_some_and(|start| g.server_now_ms() >= start);
                    if !gun_passed || igt <= 0 || !advancing {
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
