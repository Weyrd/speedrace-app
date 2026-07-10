use crate::api;
use crate::api::client::PostOutcome;
use crate::events::{SPLIT_FIRED, WS_PLAYER_RESULT};
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::{AutosplitSource, BufferedEarlySplit, PendingSplit, SharedState};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(serde::Serialize, Clone)]
struct SplitFiredPayload {
    index: u32,
    segment_ms: u64,
    new_start_ms: u64,
}

pub fn fire_split(app: &AppHandle, state: &SharedState) {
    fire_split_impl(app, state, false);
}

// skip split if player started in adavance (before official start) only for livesplit
pub fn skip_split(app: &AppHandle, state: &SharedState) {
    fire_split_impl(app, state, true);
}

enum Outcome {
    Split {
        lobby_id: String,
        split_index: u32,
        segment_name: String,
        start_ms: u64,
        end_ms: u64,
        is_final: bool,
        skip: bool,
    },
    // If split without start (only case is wasm dont expose IGT, start game/run THEN app) -> forfeit instant as we cant compute the penalty
    Forfeit {
        lobby_id: String,
    },
}

fn fire_split_impl(app: &AppHandle, state: &SharedState, force_skip: bool) {
    let now_ms = crate::autosplit::now_epoch_ms();

    let outcome = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if guard.app_state != AppState::RaceInProgress {
            return;
        }

        if guard.autosplit_source == Some(AutosplitSource::LiveSplit)
            && guard.livesplit_splits_match == Some(false)
        {
            return;
        }

        let Some(lobby) = guard.lobby.as_ref() else {
            return;
        };
        let lobby_id = lobby.lobby_id.clone();

        // if no start recorded (wasm no igt) runner have until he passes the first split to restart the run after -> forfeit
        // A catch-up skip (force_skip) never forfeits: it only advances the index.
        if guard.run_start_instant.is_none() && !force_skip {
            if guard.run_forfeited {
                return;
            }
            guard.run_forfeited = true;
            Outcome::Forfeit { lobby_id }
        } else {
            let Some(race_start_at) = guard.race_start_at else {
                return;
            };
            let end_ms = ((now_ms + guard.clock_offset_ms) - race_start_at).max(0) as u64;

            let Some(run) = guard.split_run.as_ref() else {
                return;
            };
            let seg_count = run.len() as u32;
            let index = guard.current_split_index;
            if index >= seg_count {
                return;
            }

            let segment_name = run.segment(index as usize).name().to_string();
            let start_ms = guard.segment_start_ms;

            let end_ms = end_ms.max(start_ms);

            let skip = force_skip;

            guard.current_split_index = index + 1;
            if !skip {
                guard.segment_start_ms = end_ms;
            }
            let is_final = guard.current_split_index >= seg_count;

            Outcome::Split {
                lobby_id,
                split_index: index,
                segment_name,
                start_ms,
                end_ms,
                is_final,
                skip,
            }
        }
    };

    match outcome {
        Outcome::Forfeit { lobby_id } => {
            let app = app.clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                crate::counter::flush_all_counter_buffers(&app, &state, &lobby_id).await;
                match api::lobby::post_player_forfeited(&app, &lobby_id).await {
                    Ok(result) => {
                        {
                            let Ok(mut g) = state.lock() else { return };
                            g.app_state = AppState::Finished;
                            g.race_start_at = None;
                            g.run_start_instant = None;
                        }
                        let _ = app.emit(WS_PLAYER_RESULT, result);
                    }
                    Err(e) => {
                        mlog!(
                            LogCat::Autosplit,
                            "[autosplit] unverified-start forfeit: {e}"
                        );
                        if let Ok(mut g) = state.lock() {
                            g.run_forfeited = false;
                        }
                    }
                }
            });
        }
        Outcome::Split {
            lobby_id,
            split_index,
            segment_name,
            start_ms,
            end_ms,
            is_final,
            skip,
        } => {
            if !skip {
                let _ = app.emit(
                    SPLIT_FIRED,
                    SplitFiredPayload {
                        index: split_index,
                        segment_ms: end_ms.saturating_sub(start_ms),
                        new_start_ms: end_ms,
                    },
                );
            }

            if !skip {
                enqueue_split(
                    app,
                    state,
                    PendingSplit {
                        lobby_id: lobby_id.clone(),
                        split_index,
                        segment_name,
                        start_ms,
                        end_ms,
                    },
                );
            }

            let app = app.clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                crate::counter::flush_counter_buffers(
                    &app,
                    &state,
                    &lobby_id,
                    Some(crate::api::counter_config::CounterCadence::PerSplit),
                )
                .await;

                if is_final {
                    crate::commands::lobby_commands::start_durable_finish(
                        &app, &state, lobby_id, end_ms,
                    );
                }
            });
        }
    }
}

pub fn buffer_early_split(app: &AppHandle, state: &SharedState) {
    crate::autosplit::run_started::mark_run_start(app, state, crate::autosplit::now_epoch_ms());

    let mut guard = match state.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let Some(lobby) = guard.lobby.as_ref() else {
        return;
    };
    let lobby_id = lobby.lobby_id.clone();
    let Some(run) = guard.split_run.as_ref() else {
        return;
    };
    let seg_count = run.len() as u32;
    let index = guard.current_split_index;
    if index >= seg_count {
        return;
    }
    let segment_name = run.segment(index as usize).name().to_string();
    guard.current_split_index = index + 1;
    let is_final = guard.current_split_index >= seg_count;
    guard.pending_early_splits.push(BufferedEarlySplit {
        lobby_id,
        split_index: index,
        segment_name,
        is_final,
    });
}

pub fn flush_early_splits(app: &AppHandle, state: &SharedState) {
    let buffered = {
        let mut g = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if g.autosplit_source != Some(AutosplitSource::Wasm)
            || g.app_state != AppState::RaceInProgress
            || g.pending_early_splits.is_empty()
        {
            return;
        }
        std::mem::take(&mut g.pending_early_splits)
    };

    for bs in buffered {
        emit_prestart_split(
            app,
            state,
            bs.lobby_id,
            bs.split_index,
            bs.segment_name,
            bs.is_final,
        );
    }
}

pub fn fire_prestart_split(app: &AppHandle, state: &SharedState) {
    let (lobby_id, split_index, segment_name, is_final) = {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if guard.app_state != AppState::RaceInProgress {
            return;
        }
        if guard.autosplit_source == Some(AutosplitSource::LiveSplit)
            && guard.livesplit_splits_match == Some(false)
        {
            return;
        }
        let Some(lobby) = guard.lobby.as_ref() else {
            return;
        };
        let lobby_id = lobby.lobby_id.clone();
        let Some(run) = guard.split_run.as_ref() else {
            return;
        };
        let seg_count = run.len() as u32;
        let index = guard.current_split_index;
        if index >= seg_count {
            return;
        }
        let segment_name = run.segment(index as usize).name().to_string();
        guard.current_split_index = index + 1;
        let is_final = guard.current_split_index >= seg_count;
        (lobby_id, index, segment_name, is_final)
    };

    emit_prestart_split(app, state, lobby_id, split_index, segment_name, is_final);
}

// Shared 0/0 pre-gun emit path for both sources (WASM flush + LiveSplit catch-up).
fn emit_prestart_split(
    app: &AppHandle,
    state: &SharedState,
    lobby_id: String,
    split_index: u32,
    segment_name: String,
    is_final: bool,
) {
    let _ = app.emit(
        SPLIT_FIRED,
        SplitFiredPayload {
            index: split_index,
            segment_ms: 0,
            new_start_ms: 0,
        },
    );
    enqueue_split(
        app,
        state,
        PendingSplit {
            lobby_id: lobby_id.clone(),
            split_index,
            segment_name,
            start_ms: 0,
            end_ms: 0,
        },
    );
    if is_final {
        crate::commands::lobby_commands::start_durable_finish(app, state, lobby_id, 0);
    }
}

pub fn enqueue_split(app: &AppHandle, state: &SharedState, split: PendingSplit) {
    {
        let mut guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.pending_splits.push(split);
        if guard.split_retry_running {
            return; // an existing loop will drain the queue
        }
        guard.split_retry_running = true;
    }
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        durable_split_loop(app, state).await;
    });
}

async fn durable_split_loop(app: AppHandle, state: SharedState) {
    let mut backoff = Duration::from_secs(crate::config::WS_RECONNECT_BASE_SECS);
    loop {
        let split = {
            let mut g = match state.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            match g.pending_splits.first().cloned() {
                Some(s) => s,
                None => {
                    g.split_retry_running = false;
                    return;
                }
            }
        };
        match api::lobby::submit_split(&app, &split).await {
            PostOutcome::Ok(()) | PostOutcome::Rejected => {
                if let Ok(mut g) = state.lock() {
                    if g.pending_splits.first().is_some_and(|s| {
                        s.lobby_id == split.lobby_id && s.split_index == split.split_index
                    }) {
                        g.pending_splits.remove(0);
                    }
                }
                backoff = Duration::from_secs(crate::config::WS_RECONNECT_BASE_SECS);
            }
            PostOutcome::Transient => {
                mlog!(
                    LogCat::Api,
                    "[split] back unreachable, retrying in {backoff:?}"
                );
                tokio::time::sleep(backoff).await;
                backoff =
                    (backoff * 2).min(Duration::from_secs(crate::config::WS_RECONNECT_MAX_SECS));
            }
        }
    }
}
