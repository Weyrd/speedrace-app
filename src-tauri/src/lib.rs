mod api;
mod auth;
mod commands;
mod config;
mod events;
mod lifecycle;
mod models;
mod state;
mod ws;

/// Debug macro for WS tracing.
/// Requires WS_DEBUG=true env var.
macro_rules! ws_debug {
    ($($arg:tt)*) => {
        if std::env::var("WS_DEBUG").unwrap_or_default() == "true" {
            eprintln!("[ws_debug] {}", format!($($arg)*));
        }
    };
}
pub(crate) use ws_debug;

use state::{GlobalState, SharedState};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shared_state: SharedState = Arc::new(Mutex::new(GlobalState::new()));

    tauri::Builder::default()
        .plugin(
            // force only one instance of app
            tauri_plugin_single_instance::init(|app, _argv, _cwd| {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(shared_state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_app_state,
            commands::open_login,
            commands::get_current_user,
            commands::logout,
            commands::send_stream_ready,
            commands::send_stream_stopped,
            commands::get_lobby_state,
            commands::send_player_finished,
            commands::send_player_forfeited,
            commands::acknowledge_results,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // Register deep link in DEV
            #[cfg(debug_assertions)]
            app.deep_link().register("momentum").ok();

            // Deep-link handler
            {
                let app_for_link = app_handle.clone();
                let state_for_link = shared_state.clone();

                app.deep_link().on_open_url(move |event| {
                    let app = app_for_link.clone();
                    let state = state_for_link.clone();
                    let urls: Vec<String> = event.urls().iter().map(|u| u.to_string()).collect();

                    tauri::async_runtime::spawn(async move {
                        for url in urls {
                            if url.starts_with(config::AUTH_CALLBACK_PREFIX) {
                                auth::oauth::handle_callback(app.clone(), url, state.clone()).await;
                            }
                        }
                    });
                });
            }

            {
                let app_for_restore = app_handle.clone();
                let state_for_restore = shared_state.clone();

                tauri::async_runtime::spawn(async move {
                    lifecycle::restore_session(app_for_restore, state_for_restore).await;
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
