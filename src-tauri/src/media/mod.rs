//! Media handling module
//!
//! This module handles media file operations including:
//! - Stream detection and parsing using ffprobe
//! - Stream removal using ffmpeg
//! - Media file metadata extraction
//! - FFprobe result caching for performance

mod probe_cache;
mod streams;

pub use probe_cache::{
    clear_cache as clear_probe_cache, get_cache_stats as get_probe_cache_stats, get_probe_data,
    get_probe_json, get_probe_string, invalidate_cache as invalidate_probe_cache,
};
pub use streams::{
    find_command, get_media_streams, get_search_paths, parse_disposition, parse_stream,
    remove_streams,
};
