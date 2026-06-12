mod api;
mod auth;
mod commands;
mod config;
mod events;
mod lifecycle;
mod models;
mod settings;
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

use models::AppState;
use state::{GlobalState, SharedState};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_deep_link::DeepLinkExt;

fn minimize_to_tray(window: &tauri::Window) {
    use tauri::Emitter;

    let app = window.app_handle();

    let flag = app
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("tray_hint_shown"));
    let first_time = flag.as_ref().map(|p| !p.exists()).unwrap_or(false);

    if first_time {
        if let Some(path) = &flag {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(path, b"1");
        }
        let _ = window.unminimize();
        let _ = window.emit("window:tray_hint", ());
    } else {
        let _ = window.hide();
    }
}

fn fire_finish_hotkey(app: &tauri::AppHandle) {
    let state = app.state::<SharedState>().inner().clone();

    let (lobby_id, finishing_time_ms) = {
        let guard = match state.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if guard.app_state != AppState::RaceInProgress {
            return;
        }
        let lobby_id = match &guard.lobby {
            Some(l) => l.lobby_id.clone(),
            None => return,
        };
        let start = match guard.race_start_at {
            Some(s) => s,
            None => return,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // server offset
        let elapsed = (now + guard.clock_offset_ms) - start;
        if elapsed < 0 {
            return; // still counting down
        }
        (lobby_id, elapsed as u64)
    };

    // Surface the window so the runner sees their result.
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) =
            commands::lobby_commands::finish_race(&app, &state, lobby_id, finishing_time_ms).await
        {
            eprintln!("[hotkey] finish_race error: {e}");
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shared_state: SharedState = Arc::new(Mutex::new(GlobalState::new()));

    tauri::Builder::default()
        .plugin(
            // force only one instance of app
            tauri_plugin_single_instance::init(|app, _argv, _cwd| {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.unminimize();
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
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state() == ShortcutState::Pressed {
                        fire_finish_hotkey(app);
                    }
                })
                .build(),
        )
        .manage(shared_state.clone())
        .on_window_event(|window, event| match event {
            // Close (X) quits the app
            tauri::WindowEvent::CloseRequested { .. } => {
                window.app_handle().exit(0);
            }
            // Minimize (-) sends Momentum to the tray instead of the taskbar/Dock
            tauri::WindowEvent::Resized(_) => {
                if window.is_minimized().unwrap_or(false) {
                    minimize_to_tray(window);
                }
            }
            _ => {}
        })
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
            commands::get_finish_hotkey,
            commands::set_finish_hotkey,
            commands::register_finish_hotkey,
            commands::unregister_finish_hotkey,
            commands::sync_clock,
        ])
        .setup(move |app| {
            let app_handle = app.handle().clone();

            // The finish hotkey is registered on demand when a race starts
            // (see register_finish_hotkey), so it isn't grabbed while idle.

            // System tray (Windows/Linux) macOS -> Dock,
            #[cfg(not(target_os = "macos"))]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

                let show_i = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
                let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

                TrayIconBuilder::new()
                    .icon(app.default_window_icon().unwrap().clone())
                    .tooltip("Momentum")
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "show" => {
                            if let Some(w) = app.get_webview_window("main") {
                                let _ = w.unminimize();
                                let _ = w.show();
                                let _ = w.set_focus();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app = tray.app_handle();
                            if let Some(w) = app.get_webview_window("main") {
                                if w.is_visible().unwrap_or(false) {
                                    let _ = w.hide();
                                } else {
                                    let _ = w.unminimize();
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                }
                            }
                        }
                    })
                    .build(app)?;
            }

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

            // Seed last-known offset so the hotkey is fair before the frontend re-syncs.
            if let Some((offset, _)) = settings::load_clock_offset(&app_handle) {
                if let Ok(mut guard) = shared_state.lock() {
                    guard.clock_offset_ms = offset;
                }
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
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, _event| {
            // macOS: clicking the Dock icon while the window is hidden reopens it.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = _event {
                if let Some(w) = _app_handle.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        });
}
