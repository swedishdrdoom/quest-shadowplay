//! Quest Shadowplay - Tauri Application
//!
//! This is the main entry point for the Tauri desktop/mobile app.
//! It provides a web UI for controlling the replay buffer.

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod capture;
mod commands;
mod state;

use state::AppState;
use std::sync::Arc;
use tauri::Manager;

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Quest Shadowplay...");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize application state
            let state = Arc::new(AppState::new()?);
            app.manage(state);

            log::info!("Application initialized successfully");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_recording,
            commands::stop_recording,
            commands::save_clip,
            commands::get_status,
            commands::list_clips,
            commands::delete_clip,
            commands::get_clip_thumbnail,
            commands::export_to_mp4,
            // Native hardware-accelerated recording (macOS)
            commands::start_native_recording,
            commands::stop_native_recording,
            commands::get_native_recording_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

