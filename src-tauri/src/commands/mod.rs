//! Tauri commands module
//!
//! This module re-exports all Tauri commands for the application.
//! Commands are organized by domain:
//! - File operations (list, metadata, rename, delete, move, copy)
//! - Media operations (streams, removal)
//! - Bitrate analysis (analyze, cancel, cache)
//! - System utilities (dependencies, home dir)

mod bitrate;
mod files;
mod media;

// Use wildcard re-exports to include macro-generated items from #[tauri::command]
pub use bitrate::*;
pub use files::*;
pub use media::*;
