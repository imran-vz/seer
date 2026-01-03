//! Bitrate analysis module
//!
//! This module handles bitrate analysis for media files, including:
//! - Frame-by-frame bitrate parsing using ffprobe
//! - Job queue management for analysis tasks
//! - File-based caching of analysis results

mod cache;
mod jobs;
mod parser;

pub use cache::{clear_cache, get_cached_analysis, save_to_cache};
pub use jobs::{cancel_job, complete_job, get_active_jobs, start_job, BitrateJob, JobStartResult};
pub use parser::{
    aggregate_bitrate_intervals, calculate_statistics, parse_ffprobe_frames,
    sort_streams_audio_first,
};
