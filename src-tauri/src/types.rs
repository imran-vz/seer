//! Shared types and data structures for the Seer application

use serde::{Deserialize, Serialize};

// ============================================================================
// File System Types
// ============================================================================

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mp3", "flac", "wav", "aac", "ogg",
    "wma", "m4a", "opus", "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "heic",
];

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_media: bool,
    pub size: u64,
    pub modified: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileMetadata {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified: Option<String>,
    pub created: Option<String>,
    pub is_media: bool,
    pub extension: Option<String>,
    pub ffprobe_data: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileOperationResult {
    pub success: bool,
    pub message: String,
    pub new_path: Option<String>,
}

// ============================================================================
// Media Stream Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StreamType {
    Video,
    Audio,
    Subtitle,
    Attachment,
    Data,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamInfo {
    pub index: i32,
    pub stream_type: StreamType,
    pub codec_name: Option<String>,
    pub codec_long_name: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub is_forced: bool,
    pub is_hearing_impaired: bool,
    pub is_visual_impaired: bool,
    pub is_commentary: bool,
    pub is_lyrics: bool,
    pub is_karaoke: bool,
    pub is_cover_art: bool,
    // Video specific
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub frame_rate: Option<String>,
    pub pixel_format: Option<String>,
    // Audio specific
    pub sample_rate: Option<String>,
    pub channels: Option<i32>,
    pub channel_layout: Option<String>,
    pub bit_rate: Option<String>,
    // Subtitle specific
    pub subtitle_format: Option<String>,
    // Size estimation
    pub estimated_size: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct MediaStreams {
    pub path: String,
    pub streams: Vec<StreamInfo>,
    pub video_count: usize,
    pub audio_count: usize,
    pub subtitle_count: usize,
    pub attachment_count: usize,
    pub total_size: u64,
    pub duration: f64,
}

#[derive(Debug, Serialize)]
pub struct StreamRemovalResult {
    pub success: bool,
    pub output_path: String,
    pub message: String,
}

// ============================================================================
// Bitrate Analysis Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitrateDataPoint {
    pub timestamp: f64,
    pub bitrate: u64,
    pub frame_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BitrateProgress {
    pub current: usize,
    pub total: usize,
    pub percentage: f64,
    pub stage: String,
    /// Estimated seconds remaining (if calculable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<f64>,
    /// Elapsed seconds since analysis started
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elapsed_seconds: Option<f64>,
    /// Whether sampling mode is being used (for large files)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub using_sampling: Option<bool>,
    /// Number of streams being analyzed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_count: Option<usize>,
    /// Current stream being analyzed (1-indexed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_stream: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeakInterval {
    pub start_time: f64,
    pub end_time: f64,
    pub peak_bitrate: u64,
    pub duration: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitrateStatistics {
    pub min_bitrate: u64,
    pub max_bitrate: u64,
    pub avg_bitrate: u64,
    pub median_bitrate: u64,
    pub std_deviation: f64,
    pub peak_intervals: Vec<PeakInterval>,
    pub total_frames: usize,
}

#[derive(Debug, Serialize)]
pub struct BitrateAnalysis {
    pub path: String,
    pub stream_index: i32,
    pub stream_type: StreamType,
    pub duration: f64,
    pub data_points: Vec<BitrateDataPoint>,
    pub statistics: BitrateStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamContribution {
    pub stream_index: i32,
    pub stream_type: StreamType,
    pub codec_name: String,
    pub percentage: f64,
    pub data_points: Vec<BitrateDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallBitrateAnalysis {
    pub path: String,
    pub duration: f64,
    pub data_points: Vec<BitrateDataPoint>,
    pub statistics: BitrateStatistics,
    pub stream_contributions: Vec<StreamContribution>,
    pub from_cache: bool,
}

// ============================================================================
// Job Queue Types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct JobStatus {
    pub job_id: u64,
    pub path: String,
    pub running_seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct JobInfo {
    pub job_id: u64,
    pub path: String,
    pub state: String,
    pub queued_seconds: Option<f64>,
    pub running_seconds: Option<f64>,
    pub progress_current: Option<usize>,
    pub progress_total: Option<usize>,
    pub progress_percentage: Option<f64>,
    pub progress_stage: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueueStatus {
    pub queued: Vec<JobInfo>,
    pub running: Vec<JobInfo>,
    pub max_parallel: usize,
}

// ============================================================================
// Dependency Check Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct DependencyStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DependenciesResult {
    pub all_installed: bool,
    pub dependencies: Vec<DependencyStatus>,
    pub platform: String,
}
