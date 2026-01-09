//! Tauri commands module
//!
//! This module re-exports all Tauri commands for the application.
//! Commands are organized by domain:
//! - File operations (list, metadata, rename, delete, move, copy)
//! - Media operations (streams, removal)
//! - Bitrate analysis (analyze, cancel, cache)
//! - Settings operations (get/set settings, folder picker, path validation)
//! - Installer operations (install dependencies, get strategies)
//! - System utilities (dependencies, home dir)

mod bitrate;
mod files;
mod installer;
mod media;
mod settings;

// Use wildcard re-exports to include macro-generated items from #[tauri::command]
pub use bitrate::*;
pub use files::*;
pub use installer::*;
pub use media::*;
pub use settings::*;
