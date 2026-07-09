use crate::api;
use crate::events::{SPLIT_FIRED, WS_PLAYER_RESULT};
use crate::logging::{mlog, LogCat};
use crate::models::AppState;
use crate::state::{AutosplitSource, SharedState};
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
        if guard.run_start_instant.is_none() {
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

            let app = app.clone();
            let state = state.clone();
            tauri::async_runtime::spawn(async move {
                if !skip {
                    if let Err(e) = api::lobby::post_player_split(
                        &app,
                        &lobby_id,
                        split_index,
                        segment_name,
                        start_ms,
                        end_ms,
                    )
                    .await
                    {
                        mlog!(LogCat::Autosplit, "[autosplit] post_player_split: {e}");
                    }
                }

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
