//! Media-related Tauri commands
//!
//! This module contains all Tauri commands for media operations.

use crate::media;
use crate::types::{MediaStreams, StreamRemovalResult};

#[tauri::command]
pub fn get_media_streams(path: String) -> Result<MediaStreams, String> {
    media::get_media_streams(path)
}

#[tauri::command]
pub fn remove_streams(
    path: String,
    stream_indices: Vec<i32>,
    overwrite: bool,
) -> Result<StreamRemovalResult, String> {
    media::remove_streams(path, stream_indices, overwrite)
}
