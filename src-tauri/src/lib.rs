//! Seer - Media file metadata editor and analyzer
//!
//! This is the main library entry point that exposes all modules
//! and sets up the Tauri application.

// Module declarations
pub mod bitrate;
pub mod commands;
pub mod database;
pub mod files;
pub mod media;
pub mod types;
pub mod window;

// Re-export commonly used types
pub use types::*;

use database::{get_database_url, get_migrations};

/// Run the Tauri application
pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(&get_database_url(), get_migrations())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview,
                ))
                .level(log::LevelFilter::Debug)
                .build(),
        )
        .setup(|app| {
            window::create_main_window(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // File operations
            commands::list_directory,
            commands::get_file_metadata,
            commands::get_home_dir,
            commands::check_dependencies,
            commands::rename_file,
            commands::delete_file,
            commands::move_file,
            commands::copy_file,
            commands::create_folder,
            commands::reveal_in_folder,
            // Media operations
            commands::get_media_streams,
            commands::remove_streams,
            // Bitrate analysis
            commands::analyze_stream_bitrate,
            commands::analyze_overall_bitrate,
            commands::cancel_bitrate_analysis,
            commands::get_bitrate_job_status,
            commands::clear_bitrate_cache_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
