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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FileEntry;

    // ========== FilterCriteria tests ==========

    #[test]
    fn test_filter_criteria_default_values() {
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(criteria.size_min.is_none());
        assert!(criteria.size_max.is_none());
        assert!(criteria.date_min.is_none());
        assert!(criteria.date_max.is_none());
        assert!(criteria.extensions.is_empty());
        assert!(criteria.media_filters.is_none());
        assert!(criteria.file_type.is_none());
    }

    #[test]
    fn test_filter_criteria_with_size_range() {
        let criteria = FilterCriteria {
            size_min: Some(1024),
            size_max: Some(1048576),
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert_eq!(criteria.size_min, Some(1024));
        assert_eq!(criteria.size_max, Some(1048576));
    }

    #[test]
    fn test_filter_criteria_with_extensions() {
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec!["mp4".to_string(), "mkv".to_string()],
            media_filters: None,
            file_type: None,
        };
        assert_eq!(criteria.extensions.len(), 2);
        assert!(criteria.extensions.contains(&"mp4".to_string()));
        assert!(criteria.extensions.contains(&"mkv".to_string()));
    }

    // ========== FileTypeFilter tests ==========

    #[test]
    fn test_file_type_filter_all() {
        let filter = FileTypeFilter::All;
        match filter {
            FileTypeFilter::All => assert!(true),
            _ => panic!("Expected All"),
        }
    }

    #[test]
    fn test_file_type_filter_files_only() {
        let filter = FileTypeFilter::FilesOnly;
        match filter {
            FileTypeFilter::FilesOnly => assert!(true),
            _ => panic!("Expected FilesOnly"),
        }
    }

    #[test]
    fn test_file_type_filter_directories_only() {
        let filter = FileTypeFilter::DirectoriesOnly;
        match filter {
            FileTypeFilter::DirectoriesOnly => assert!(true),
            _ => panic!("Expected DirectoriesOnly"),
        }
    }

    #[test]
    fn test_file_type_filter_media_only() {
        let filter = FileTypeFilter::MediaOnly;
        match filter {
            FileTypeFilter::MediaOnly => assert!(true),
            _ => panic!("Expected MediaOnly"),
        }
    }

    // ========== MediaFilters tests ==========

    #[test]
    fn test_media_filters_empty() {
        let filters = MediaFilters {
            video_codecs: vec![],
            audio_codecs: vec![],
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            min_duration: None,
            max_duration: None,
        };
        assert!(filters.video_codecs.is_empty());
        assert!(filters.audio_codecs.is_empty());
    }

    #[test]
    fn test_media_filters_with_codecs() {
        let filters = MediaFilters {
            video_codecs: vec!["h264".to_string(), "hevc".to_string()],
            audio_codecs: vec!["aac".to_string()],
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            min_duration: None,
            max_duration: None,
        };
        assert_eq!(filters.video_codecs.len(), 2);
        assert_eq!(filters.audio_codecs.len(), 1);
    }

    #[test]
    fn test_media_filters_with_resolution() {
        let filters = MediaFilters {
            video_codecs: vec![],
            audio_codecs: vec![],
            min_width: Some(1920),
            max_width: Some(3840),
            min_height: Some(1080),
            max_height: Some(2160),
            min_duration: None,
            max_duration: None,
        };
        assert_eq!(filters.min_width, Some(1920));
        assert_eq!(filters.max_width, Some(3840));
        assert_eq!(filters.min_height, Some(1080));
        assert_eq!(filters.max_height, Some(2160));
    }

    #[test]
    fn test_media_filters_with_duration() {
        let filters = MediaFilters {
            video_codecs: vec![],
            audio_codecs: vec![],
            min_width: None,
            max_width: None,
            min_height: None,
            max_height: None,
            min_duration: Some(60.0),
            max_duration: Some(3600.0),
        };
        assert_eq!(filters.min_duration, Some(60.0));
        assert_eq!(filters.max_duration, Some(3600.0));
    }

    // ========== FilteredFileEntry tests ==========

    #[test]
    fn test_filtered_file_entry_non_media() {
        let file = FileEntry {
            name: "document.txt".to_string(),
            path: "/path/document.txt".to_string(),
            is_dir: false,
            is_media: false,
            size: 1024,
            modified: Some("2024-01-15 10:30:00".to_string()),
        };
        let filtered = FilteredFileEntry {
            file,
            video_codec: None,
            audio_codec: None,
            width: None,
            height: None,
            duration: None,
        };
        assert_eq!(filtered.file.name, "document.txt");
        assert!(filtered.video_codec.is_none());
    }

    #[test]
    fn test_filtered_file_entry_media() {
        let file = FileEntry {
            name: "video.mp4".to_string(),
            path: "/path/video.mp4".to_string(),
            is_dir: false,
            is_media: true,
            size: 104857600,
            modified: Some("2024-01-15 10:30:00".to_string()),
        };
        let filtered = FilteredFileEntry {
            file,
            video_codec: Some("h264".to_string()),
            audio_codec: Some("aac".to_string()),
            width: Some(1920),
            height: Some(1080),
            duration: Some(3600.5),
        };
        assert_eq!(filtered.file.name, "video.mp4");
        assert_eq!(filtered.video_codec, Some("h264".to_string()));
        assert_eq!(filtered.audio_codec, Some("aac".to_string()));
        assert_eq!(filtered.width, Some(1920));
        assert_eq!(filtered.height, Some(1080));
        assert_eq!(filtered.duration, Some(3600.5));
    }

    // ========== FilterResult tests ==========

    #[test]
    fn test_filter_result_empty() {
        let result = FilterResult {
            files: vec![],
            total_count: 0,
            filtered_count: 0,
        };
        assert!(result.files.is_empty());
        assert_eq!(result.total_count, 0);
        assert_eq!(result.filtered_count, 0);
    }

    #[test]
    fn test_filter_result_with_files() {
        let file = FileEntry {
            name: "test.mp4".to_string(),
            path: "/path/test.mp4".to_string(),
            is_dir: false,
            is_media: true,
            size: 1024,
            modified: None,
        };
        let filtered = FilteredFileEntry {
            file,
            video_codec: None,
            audio_codec: None,
            width: None,
            height: None,
            duration: None,
        };
        let result = FilterResult {
            files: vec![filtered],
            total_count: 10,
            filtered_count: 1,
        };
        assert_eq!(result.files.len(), 1);
        assert_eq!(result.total_count, 10);
        assert_eq!(result.filtered_count, 1);
    }

    // ========== parse_date tests ==========

    #[test]
    fn test_parse_date_valid_start_of_day() {
        let date = parse_date("2024-01-15", false);
        assert!(date.is_some());
        let dt = date.unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2024-01-15 00:00:00"
        );
    }

    #[test]
    fn test_parse_date_valid_end_of_day() {
        let date = parse_date("2024-01-15", true);
        assert!(date.is_some());
        let dt = date.unwrap();
        assert_eq!(
            dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2024-01-15 23:59:59"
        );
    }

    #[test]
    fn test_parse_date_invalid_format() {
        assert!(parse_date("01-15-2024", false).is_none());
        assert!(parse_date("2024/01/15", false).is_none());
        assert!(parse_date("not-a-date", false).is_none());
    }

    #[test]
    fn test_parse_date_edge_cases() {
        // First day of year
        assert!(parse_date("2024-01-01", false).is_some());
        // Last day of year
        assert!(parse_date("2024-12-31", true).is_some());
        // Leap year
        assert!(parse_date("2024-02-29", false).is_some());
    }

    // ========== parse_modified_time tests ==========

    #[test]
    fn test_parse_modified_time_valid() {
        let time = parse_modified_time("2024-01-15 10:30:45");
        assert!(time.is_some());
    }

    #[test]
    fn test_parse_modified_time_invalid() {
        assert!(parse_modified_time("invalid").is_none());
        assert!(parse_modified_time("2024-01-15").is_none());
    }

    // ========== matches_criteria tests ==========

    fn make_test_file(name: &str, is_dir: bool, is_media: bool, size: u64) -> FileEntry {
        FileEntry {
            name: name.to_string(),
            path: format!("/test/{}", name),
            is_dir,
            is_media,
            size,
            modified: Some("2024-06-15 12:00:00".to_string()),
        }
    }

    #[test]
    fn test_matches_criteria_no_filters() {
        let file = make_test_file("test.mp4", false, true, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(matches_criteria(&file, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_file_type_files_only() {
        let file = make_test_file("test.mp4", false, true, 1024);
        let dir = make_test_file("folder", true, false, 0);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: Some(FileTypeFilter::FilesOnly),
        };
        assert!(matches_criteria(&file, &criteria, None));
        assert!(!matches_criteria(&dir, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_file_type_directories_only() {
        let file = make_test_file("test.mp4", false, true, 1024);
        let dir = make_test_file("folder", true, false, 0);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: Some(FileTypeFilter::DirectoriesOnly),
        };
        assert!(!matches_criteria(&file, &criteria, None));
        assert!(matches_criteria(&dir, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_file_type_media_only() {
        let media = make_test_file("video.mp4", false, true, 1024);
        let doc = make_test_file("doc.txt", false, false, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: Some(FileTypeFilter::MediaOnly),
        };
        assert!(matches_criteria(&media, &criteria, None));
        assert!(!matches_criteria(&doc, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_size_min() {
        let small = make_test_file("small.mp4", false, true, 500);
        let large = make_test_file("large.mp4", false, true, 2000);
        let criteria = FilterCriteria {
            size_min: Some(1000),
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(!matches_criteria(&small, &criteria, None));
        assert!(matches_criteria(&large, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_size_max() {
        let small = make_test_file("small.mp4", false, true, 500);
        let large = make_test_file("large.mp4", false, true, 2000);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: Some(1000),
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(matches_criteria(&small, &criteria, None));
        assert!(!matches_criteria(&large, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_size_range() {
        let small = make_test_file("small.mp4", false, true, 500);
        let medium = make_test_file("medium.mp4", false, true, 1500);
        let large = make_test_file("large.mp4", false, true, 2500);
        let criteria = FilterCriteria {
            size_min: Some(1000),
            size_max: Some(2000),
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(!matches_criteria(&small, &criteria, None));
        assert!(matches_criteria(&medium, &criteria, None));
        assert!(!matches_criteria(&large, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_size_ignores_directories() {
        let dir = make_test_file("folder", true, false, 0);
        let criteria = FilterCriteria {
            size_min: Some(1000),
            size_max: Some(2000),
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        // Directories should not be filtered by size
        assert!(matches_criteria(&dir, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_extension_single() {
        let mp4 = make_test_file("video.mp4", false, true, 1024);
        let mkv = make_test_file("video.mkv", false, true, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec!["mp4".to_string()],
            media_filters: None,
            file_type: None,
        };
        assert!(matches_criteria(&mp4, &criteria, None));
        assert!(!matches_criteria(&mkv, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_extension_multiple() {
        let mp4 = make_test_file("video.mp4", false, true, 1024);
        let mkv = make_test_file("video.mkv", false, true, 1024);
        let avi = make_test_file("video.avi", false, true, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec!["mp4".to_string(), "mkv".to_string()],
            media_filters: None,
            file_type: None,
        };
        assert!(matches_criteria(&mp4, &criteria, None));
        assert!(matches_criteria(&mkv, &criteria, None));
        assert!(!matches_criteria(&avi, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_extension_case_insensitive() {
        let upper = make_test_file("video.MP4", false, true, 1024);
        let lower = make_test_file("video.mp4", false, true, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec!["mp4".to_string()],
            media_filters: None,
            file_type: None,
        };
        assert!(matches_criteria(&upper, &criteria, None));
        assert!(matches_criteria(&lower, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_extension_ignores_directories() {
        let dir = make_test_file("folder.mp4", true, false, 0);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec!["mkv".to_string()],
            media_filters: None,
            file_type: None,
        };
        // Directory named folder.mp4 should pass extension filter
        assert!(matches_criteria(&dir, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_date_range() {
        let file = FileEntry {
            name: "test.mp4".to_string(),
            path: "/test/test.mp4".to_string(),
            is_dir: false,
            is_media: true,
            size: 1024,
            modified: Some("2024-06-15 12:00:00".to_string()),
        };
        // Within range
        let within_criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: Some("2024-01-01".to_string()),
            date_max: Some("2024-12-31".to_string()),
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(matches_criteria(&file, &within_criteria, None));

        // Before range
        let before_criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: Some("2024-07-01".to_string()),
            date_max: None,
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(!matches_criteria(&file, &before_criteria, None));

        // After range
        let after_criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: Some("2024-05-01".to_string()),
            extensions: vec![],
            media_filters: None,
            file_type: None,
        };
        assert!(!matches_criteria(&file, &after_criteria, None));
    }

    #[test]
    fn test_matches_criteria_combined_filters() {
        let file = make_test_file("video.mp4", false, true, 1500);
        let criteria = FilterCriteria {
            size_min: Some(1000),
            size_max: Some(2000),
            date_min: None,
            date_max: None,
            extensions: vec!["mp4".to_string()],
            media_filters: None,
            file_type: Some(FileTypeFilter::MediaOnly),
        };
        assert!(matches_criteria(&file, &criteria, None));
    }

    #[test]
    fn test_matches_criteria_media_filters_video_codec() {
        let file = make_test_file("video.mp4", false, true, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: Some(MediaFilters {
                video_codecs: vec!["h264".to_string()],
                audio_codecs: vec![],
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                min_duration: None,
                max_duration: None,
            }),
            file_type: None,
        };
        let media_info = Some((
            Some("h264".to_string()),
            Some("aac".to_string()),
            Some(1920),
            Some(1080),
            Some(120.0),
        ));
        assert!(matches_criteria(&file, &criteria, media_info));

        let wrong_codec_info = Some((
            Some("hevc".to_string()),
            Some("aac".to_string()),
            Some(1920),
            Some(1080),
            Some(120.0),
        ));
        assert!(!matches_criteria(&file, &criteria, wrong_codec_info));
    }

    #[test]
    fn test_matches_criteria_media_filters_resolution() {
        let file = make_test_file("video.mp4", false, true, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: Some(MediaFilters {
                video_codecs: vec![],
                audio_codecs: vec![],
                min_width: Some(1920),
                max_width: None,
                min_height: Some(1080),
                max_height: None,
                min_duration: None,
                max_duration: None,
            }),
            file_type: None,
        };
        let hd_info = Some((
            Some("h264".to_string()),
            Some("aac".to_string()),
            Some(1920),
            Some(1080),
            Some(120.0),
        ));
        assert!(matches_criteria(&file, &criteria, hd_info));

        let sd_info = Some((
            Some("h264".to_string()),
            Some("aac".to_string()),
            Some(1280),
            Some(720),
            Some(120.0),
        ));
        assert!(!matches_criteria(&file, &criteria, sd_info));
    }

    #[test]
    fn test_matches_criteria_media_filters_duration() {
        let file = make_test_file("video.mp4", false, true, 1024);
        let criteria = FilterCriteria {
            size_min: None,
            size_max: None,
            date_min: None,
            date_max: None,
            extensions: vec![],
            media_filters: Some(MediaFilters {
                video_codecs: vec![],
                audio_codecs: vec![],
                min_width: None,
                max_width: None,
                min_height: None,
                max_height: None,
                min_duration: Some(60.0),
                max_duration: Some(600.0),
            }),
            file_type: None,
        };
        let within_info = Some((
            Some("h264".to_string()),
            Some("aac".to_string()),
            Some(1920),
            Some(1080),
            Some(300.0),
        ));
        assert!(matches_criteria(&file, &criteria, within_info));

        let too_short = Some((
            Some("h264".to_string()),
            Some("aac".to_string()),
            Some(1920),
            Some(1080),
            Some(30.0),
        ));
        assert!(!matches_criteria(&file, &criteria, too_short));

        let too_long = Some((
            Some("h264".to_string()),
            Some("aac".to_string()),
            Some(1920),
            Some(1080),
            Some(900.0),
        ));
        assert!(!matches_criteria(&file, &criteria, too_long));
    }
}
