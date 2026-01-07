//! Smart file filtering system
//!
//! Filters file entries by various criteria including:
//! - Size range
//! - Date modified range
//! - Extensions
//! - Media properties (codec, resolution, bitrate)

use crate::config;
use crate::files::operations::{format_time, is_media_file};
use crate::media::get_probe_data;
use crate::types::FileEntry;
use chrono::{DateTime, Local, NaiveDate, TimeZone};
use log::debug;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Filter criteria for file listing
#[derive(Debug, Clone, serde::Deserialize)]
pub struct FilterCriteria {
    /// Minimum file size in bytes (inclusive)
    #[serde(default)]
    pub size_min: Option<u64>,
    /// Maximum file size in bytes (inclusive)
    #[serde(default)]
    pub size_max: Option<u64>,
    /// Minimum date (YYYY-MM-DD format, inclusive)
    #[serde(default)]
    pub date_min: Option<String>,
    /// Maximum date (YYYY-MM-DD format, inclusive)
    #[serde(default)]
    pub date_max: Option<String>,
    /// File extensions to include (empty = all)
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Media-specific filters
    #[serde(default)]
    pub media_filters: Option<MediaFilters>,
    /// File type filter
    #[serde(default)]
    pub file_type: Option<FileTypeFilter>,
}

/// File type filter options
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileTypeFilter {
    #[default]
    All,
    FilesOnly,
    DirectoriesOnly,
    MediaOnly,
}

/// Media-specific filter criteria
#[derive(Debug, Clone, serde::Deserialize)]
pub struct MediaFilters {
    /// Video codecs to include (empty = all)
    #[serde(default)]
    pub video_codecs: Vec<String>,
    /// Audio codecs to include (empty = all)
    #[serde(default)]
    pub audio_codecs: Vec<String>,
    /// Minimum video width
    #[serde(default)]
    pub min_width: Option<i32>,
    /// Maximum video width
    #[serde(default)]
    pub max_width: Option<i32>,
    /// Minimum video height
    #[serde(default)]
    pub min_height: Option<i32>,
    /// Maximum video height
    #[serde(default)]
    pub max_height: Option<i32>,
    /// Minimum duration in seconds
    #[serde(default)]
    pub min_duration: Option<f64>,
    /// Maximum duration in seconds
    #[serde(default)]
    pub max_duration: Option<f64>,
}

/// Filtered file entry with additional media info
#[derive(Debug, Clone, serde::Serialize)]
pub struct FilteredFileEntry {
    #[serde(flatten)]
    pub file: FileEntry,
    /// Video codec (if media file)
    pub video_codec: Option<String>,
    /// Audio codec (if media file)
    pub audio_codec: Option<String>,
    /// Video width (if media file)
    pub width: Option<i32>,
    /// Video height (if media file)
    pub height: Option<i32>,
    /// Duration in seconds (if media file)
    pub duration: Option<f64>,
}

/// Result of filter operation
#[derive(Debug, serde::Serialize)]
pub struct FilterResult {
    pub files: Vec<FilteredFileEntry>,
    pub total_count: usize,
    pub filtered_count: usize,
}

/// Parse date string in YYYY-MM-DD format to DateTime<Local>
fn parse_date(date_str: &str, end_of_day: bool) -> Option<DateTime<Local>> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .ok()
        .and_then(|date| {
            let time = if end_of_day {
                chrono::NaiveTime::from_hms_opt(23, 59, 59)?
            } else {
                chrono::NaiveTime::from_hms_opt(0, 0, 0)?
            };
            let naive_datetime = date.and_time(time);
            Local.from_local_datetime(&naive_datetime).single()
        })
}

/// Parse file modified time string to DateTime<Local>
fn parse_modified_time(modified: &str) -> Option<DateTime<Local>> {
    DateTime::parse_from_str(&format!("{} +0000", modified), "%Y-%m-%d %H:%M:%S %z")
        .ok()
        .map(|dt| dt.with_timezone(&Local))
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(modified, "%Y-%m-%d %H:%M:%S")
                .ok()
                .and_then(|naive| Local.from_local_datetime(&naive).single())
        })
}

/// Get media info from probe cache for filtering
fn get_media_info(
    path: &str,
) -> Option<(
    Option<String>,
    Option<String>,
    Option<i32>,
    Option<i32>,
    Option<f64>,
)> {
    match get_probe_data(path) {
        Ok((_, data, _)) => {
            let mut video_codec: Option<String> = None;
            let mut audio_codec: Option<String> = None;
            let mut width: Option<i32> = None;
            let mut height: Option<i32> = None;

            // Get duration from format
            let duration: Option<f64> = data
                .get("format")
                .and_then(|f: &Value| f.get("duration"))
                .and_then(|d: &Value| d.as_str())
                .and_then(|s: &str| s.parse::<f64>().ok());

            // Parse streams
            if let Some(streams) = data.get("streams").and_then(|s: &Value| s.as_array()) {
                for stream in streams {
                    let codec_type: Option<&str> =
                        stream.get("codec_type").and_then(|t: &Value| t.as_str());
                    let codec_name: Option<&str> =
                        stream.get("codec_name").and_then(|c: &Value| c.as_str());

                    match codec_type {
                        Some("video") => {
                            if video_codec.is_none() {
                                video_codec = codec_name.map(|s: &str| s.to_string());
                                width = stream
                                    .get("width")
                                    .and_then(|w: &Value| w.as_i64())
                                    .map(|w: i64| w as i32);
                                height = stream
                                    .get("height")
                                    .and_then(|h: &Value| h.as_i64())
                                    .map(|h: i64| h as i32);
                            }
                        }
                        Some("audio") => {
                            if audio_codec.is_none() {
                                audio_codec = codec_name.map(|s: &str| s.to_string());
                            }
                        }
                        _ => {}
                    }
                }
            }

            Some((video_codec, audio_codec, width, height, duration))
        }
        Err(e) => {
            debug!("Failed to get media info for {}: {}", path, e);
            None
        }
    }
}

/// Check if a file matches the filter criteria
fn matches_criteria(
    file: &FileEntry,
    criteria: &FilterCriteria,
    media_info: Option<(
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<i32>,
        Option<f64>,
    )>,
) -> bool {
    // File type filter
    if let Some(ref file_type) = criteria.file_type {
        match file_type {
            FileTypeFilter::FilesOnly if file.is_dir => return false,
            FileTypeFilter::DirectoriesOnly if !file.is_dir => return false,
            FileTypeFilter::MediaOnly if !file.is_media => return false,
            _ => {}
        }
    }

    // Size filter (skip for directories)
    if !file.is_dir {
        if let Some(min) = criteria.size_min {
            if file.size < min {
                return false;
            }
        }
        if let Some(max) = criteria.size_max {
            if file.size > max {
                return false;
            }
        }
    }

    // Date filter
    if criteria.date_min.is_some() || criteria.date_max.is_some() {
        if let Some(ref modified) = file.modified {
            if let Some(file_date) = parse_modified_time(modified) {
                if let Some(ref min_str) = criteria.date_min {
                    if let Some(min_date) = parse_date(min_str, false) {
                        if file_date < min_date {
                            return false;
                        }
                    }
                }
                if let Some(ref max_str) = criteria.date_max {
                    if let Some(max_date) = parse_date(max_str, true) {
                        if file_date > max_date {
                            return false;
                        }
                    }
                }
            }
        } else {
            // No modified date available, exclude if date filter is set
            if criteria.date_min.is_some() || criteria.date_max.is_some() {
                return false;
            }
        }
    }

    // Extension filter (skip for directories)
    if !file.is_dir && !criteria.extensions.is_empty() {
        let ext = Path::new(&file.path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let ext_set: HashSet<String> = criteria
            .extensions
            .iter()
            .map(|e| e.to_lowercase())
            .collect();
        if !ext_set.contains(&ext) {
            return false;
        }
    }

    // Media filters (only apply to media files)
    if file.is_media {
        if let Some(ref media_filters) = criteria.media_filters {
            if let Some((video_codec, audio_codec, width, height, duration)) = media_info {
                // Video codec filter
                if !media_filters.video_codecs.is_empty() {
                    if let Some(ref vc) = video_codec {
                        let codec_set: HashSet<String> = media_filters
                            .video_codecs
                            .iter()
                            .map(|c| c.to_lowercase())
                            .collect();
                        if !codec_set.contains(&vc.to_lowercase()) {
                            return false;
                        }
                    } else {
                        return false; // No video codec but filter requires one
                    }
                }

                // Audio codec filter
                if !media_filters.audio_codecs.is_empty() {
                    if let Some(ref ac) = audio_codec {
                        let codec_set: HashSet<String> = media_filters
                            .audio_codecs
                            .iter()
                            .map(|c| c.to_lowercase())
                            .collect();
                        if !codec_set.contains(&ac.to_lowercase()) {
                            return false;
                        }
                    } else {
                        return false; // No audio codec but filter requires one
                    }
                }

                // Width filter
                if let Some(min_width) = media_filters.min_width {
                    if width.map(|w| w < min_width).unwrap_or(true) {
                        return false;
                    }
                }
                if let Some(max_width) = media_filters.max_width {
                    if width.map(|w| w > max_width).unwrap_or(true) {
                        return false;
                    }
                }

                // Height filter
                if let Some(min_height) = media_filters.min_height {
                    if height.map(|h| h < min_height).unwrap_or(true) {
                        return false;
                    }
                }
                if let Some(max_height) = media_filters.max_height {
                    if height.map(|h| h > max_height).unwrap_or(true) {
                        return false;
                    }
                }

                // Duration filter
                if let Some(min_duration) = media_filters.min_duration {
                    if duration.map(|d| d < min_duration).unwrap_or(true) {
                        return false;
                    }
                }
                if let Some(max_duration) = media_filters.max_duration {
                    if duration.map(|d| d > max_duration).unwrap_or(true) {
                        return false;
                    }
                }
            } else {
                // No media info available but media filter set
                return false;
            }
        }
    }

    true
}

/// Apply filters to a directory listing
pub fn apply_filters(current_dir: String, filters: FilterCriteria) -> Result<FilterResult, String> {
    debug!("apply_filters: dir={}, filters={:?}", current_dir, filters);

    let dir_path = Path::new(&current_dir);
    let validated_path = config::validate_path(dir_path)?;

    let entries =
        fs::read_dir(&validated_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    let mut all_files: Vec<FileEntry> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().ok();
        let is_dir = path.is_dir();
        let is_media = !is_dir && is_media_file(&path);

        all_files.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            is_media,
            size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            modified: metadata.and_then(|m| format_time(m.modified())),
        });
    }

    let total_count = all_files.len();

    // Apply filters
    let filtered_files: Vec<FilteredFileEntry> = all_files
        .into_iter()
        .filter_map(|file| {
            // Get media info if needed for filtering
            let media_info = if file.is_media && (filters.media_filters.is_some()) {
                get_media_info(&file.path)
            } else {
                None
            };

            if matches_criteria(&file, &filters, media_info.clone()) {
                // Get media info for display if not already fetched
                let (video_codec, audio_codec, width, height, duration) = if file.is_media {
                    media_info.unwrap_or_else(|| {
                        get_media_info(&file.path).unwrap_or((None, None, None, None, None))
                    })
                } else {
                    (None, None, None, None, None)
                };

                Some(FilteredFileEntry {
                    file,
                    video_codec,
                    audio_codec,
                    width,
                    height,
                    duration,
                })
            } else {
                None
            }
        })
        .collect();

    // Sort: directories first, then by name
    let mut result = filtered_files;
    result.sort_by(|a, b| match (a.file.is_dir, b.file.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.file.name.to_lowercase().cmp(&b.file.name.to_lowercase()),
    });

    let filtered_count = result.len();
    debug!(
        "Filtered {} files to {} results",
        total_count, filtered_count
    );

    Ok(FilterResult {
        files: result,
        total_count,
        filtered_count,
    })
}

/// Get available extensions in a directory
pub fn get_available_extensions(current_dir: String) -> Result<Vec<String>, String> {
    let dir_path = Path::new(&current_dir);
    let validated_path = config::validate_path(dir_path)?;

    let entries =
        fs::read_dir(&validated_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    let mut extensions: HashSet<String> = HashSet::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                extensions.insert(ext.to_lowercase());
            }
        }
    }

    let mut result: Vec<String> = extensions.into_iter().collect();
    result.sort();
    Ok(result)
}
