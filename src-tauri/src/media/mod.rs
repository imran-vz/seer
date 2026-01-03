//! Media handling module
//!
//! This module handles media file operations including:
//! - Stream detection and parsing using ffprobe
//! - Stream removal using ffmpeg
//! - Media file metadata extraction

mod streams;

pub use streams::{
    find_command, get_media_streams, get_search_paths, parse_disposition, parse_stream,
    remove_streams,
};
