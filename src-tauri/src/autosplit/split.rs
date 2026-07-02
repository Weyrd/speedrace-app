use crate::api;
use crate::events::{SPLIT_FIRED, WS_PLAYER_RESULT};
use crate::models::AppState;
use crate::state::{AutosplitSource, SharedState};
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

#[derive(serde::Serialize, Clone)]
struct SplitFiredPayload {
    index: u32,
    segment_ms: u64,
    new_start_ms: u64,
}

pub fn fire_split(app: &AppHandle, state: &SharedState) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    let result = {
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
        let Some(lobby) = guard.lobby.as_ref() else {
            return;
        };
        let lobby_id = lobby.lobby_id.clone();
        let start_ms = guard.segment_start_ms;

        guard.current_split_index = index + 1;
        guard.segment_start_ms = end_ms;
        let is_final = guard.current_split_index >= seg_count;

        (lobby_id, index, segment_name, start_ms, end_ms, is_final)
    };

    let (lobby_id, split_index, segment_name, start_ms, end_ms, is_final) = result;

    let _ = app.emit(
        SPLIT_FIRED,
        SplitFiredPayload {
            index: split_index,
            segment_ms: end_ms.saturating_sub(start_ms),
            new_start_ms: end_ms,
        },
    );

    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
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
            eprintln!("[autosplit] post_player_split: {e}");
        }

        if is_final {
            match api::lobby::post_player_finished(&app, &lobby_id, end_ms).await {
                Ok(player_result) => {
                    {
                        let mut guard = match state.lock() {
                            Ok(g) => g,
                            Err(_) => return,
                        };
                        guard.app_state = AppState::Finished;
                        guard.race_start_at = None;
                        guard.autosplitter_cancel.store(true, Ordering::SeqCst);
                    }
                    let _ = app.emit(WS_PLAYER_RESULT, player_result);
                    // Don't steal focus from the game on auto-finish; flash the taskbar instead.
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.request_user_attention(Some(
                            tauri::UserAttentionType::Informational,
                        ));
                    }
                }
                Err(e) => eprintln!("[autosplit] post_player_finished: {e}"),
            }
        }
    });
}
