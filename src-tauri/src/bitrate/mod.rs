//! Bitrate analysis module
//!
//! This module handles bitrate analysis for media files, including:
//! - Frame-by-frame bitrate parsing using ffprobe
//! - File hash computation for cache validation
//!
//! Note: Caching is now handled by the frontend via SQLite database.
//! The backend focuses on analysis and provides hash computation for
//! cache validation. Job queue management is handled by the centralized jobs module.

mod cache;
mod parser;

pub use cache::{clear_cache, compute_file_hash, get_cached_analysis, save_to_cache};
pub use parser::{
    aggregate_bitrate_intervals, calculate_statistics, extrapolate_sampled_data,
    parse_ffprobe_auto, parse_ffprobe_frames, parse_ffprobe_packets, parse_ffprobe_sampled,
    sort_streams_audio_first, SAMPLE_COUNT, SAMPLE_DURATION_SECS, SAMPLING_THRESHOLD_BYTES,
};
