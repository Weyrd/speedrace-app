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

// Advance the split index without recording a segment (burst intermediate, true time unknowable).
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
    // Split fired with no start captured (mid-run attach, no edge/IGT): forfeit, don't record.
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

        // No start captured: the run's beginning was never seen (the runner had until this first
        // split to reset), so forfeit instead of recording. Latch so repeated fires don't
        // re-forfeit; a reset() clears run_forfeited and re-arms.
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

            // Clamp up: a backward clock re-sync can push raw end below start; keep it monotonic so
            // a post-gun split records at-worst-zero duration and isn't dropped as non-monotonic.
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
                // Flush buffered counters before the terminal POST (player-counters rule).
                crate::counter::flush_all_counter_buffers(&app, &state, &lobby_id).await;
                match api::lobby::post_player_forfeited(&app, &lobby_id).await {
                    Ok(result) => {
                        {
                            let Ok(mut g) = state.lock() else { return };
                            g.app_state = AppState::Finished;
                            g.race_start_at = None;
                            g.run_start_instant = None;
                        }
                        // Mirror send_player_forfeited: move the frontend off Racing.
                        let _ = app.emit(WS_PLAYER_RESULT, result);
                    }
                    Err(e) => {
                        mlog!(
                            LogCat::Autosplit,
                            "[autosplit] unverified-start forfeit: {e}"
                        );
                        // Un-latch so the next split re-attempts the forfeit.
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

                // The split just advanced; ship PerSplit-cadence buffers for the segment that ended.
                crate::counter::flush_counter_buffers(
                    &app,
                    &state,
                    &lobby_id,
                    Some(crate::api::counter_config::CounterCadence::PerSplit),
                )
                .await;

                if is_final {
                    // Durable: retried until the back acks, so a mid-race outage can't lose it.
                    crate::commands::lobby_commands::start_durable_finish(
                        &app, &state, lobby_id, end_ms,
                    );
                }
            });
        }
    }
}
