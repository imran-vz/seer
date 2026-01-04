//! Bitrate analysis module
//!
//! This module handles bitrate analysis for media files, including:
//! - Frame-by-frame bitrate parsing using ffprobe
//! - Job queue management for analysis tasks
//! - File hash computation for cache validation
//!
//! Note: Caching is now handled by the frontend via SQLite database.
//! The backend focuses on analysis and provides hash computation for
//! cache validation.

mod cache;
mod jobs;
mod parser;

pub use cache::{clear_cache, compute_file_hash, get_cached_analysis, save_to_cache};
pub use jobs::{
    cancel_all_jobs, cancel_job, complete_job, enqueue_job, get_active_jobs,
    get_job_cancel_flag, get_queue_status, set_max_parallel_jobs, JobStartResult,
};
pub use parser::{
    aggregate_bitrate_intervals, calculate_statistics, parse_ffprobe_frames,
    sort_streams_audio_first,
};
