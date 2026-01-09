//! Media stream detection and manipulation
//!
//! This module handles:
//! - Finding ffmpeg/ffprobe commands
//! - Parsing stream information from media files
//! - Removing streams from media files

use log::debug;
use serde_json;
use std::fs;
use std::path::Path;
use std::process::Command;

use super::probe_cache;
use crate::config;
use crate::types::{MediaStreams, StreamInfo, StreamRemovalResult, StreamType};

/// Get common search paths for finding executables
pub fn get_search_paths() -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();

    // PRIORITY: Check Seer's app data bin directory first (for directly installed tools)
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            paths.push(format!(r"{}\Seer\bin", appdata));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            paths.push(format!("{}/Library/Application Support/Seer/bin", home));
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            paths.push(format!("{}/.local/share/Seer/bin", home));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // Windows common paths
        paths.extend([
            r"C:\Program Files\ffmpeg\bin".to_string(),
            r"C:\Program Files (x86)\ffmpeg\bin".to_string(),
            r"C:\ffmpeg\bin".to_string(),
            r"C:\Program Files\exiftool".to_string(),
            r"C:\Program Files (x86)\exiftool".to_string(),
        ]);

        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            paths.push(format!(r"{}\Microsoft\WindowsApps", local_app_data));
        }
        if let Ok(program_data) = std::env::var("ProgramData") {
            paths.push(format!(r"{}\chocolatey\bin", program_data));
        }
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            paths.push(format!(r"{}\scoop\shims", userprofile));
            paths.push(format!(r"{}\.local\bin", userprofile));
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS and Linux common paths
        paths.extend([
            "/usr/local/bin".to_string(),
            "/usr/bin".to_string(),
            "/bin".to_string(),
            "/opt/homebrew/bin".to_string(), // macOS Apple Silicon Homebrew
            "/opt/local/bin".to_string(),    // MacPorts
            "/snap/bin".to_string(),         // Linux Snap packages
            "/var/lib/flatpak/exports/bin".to_string(), // Linux Flatpak
        ]);

        if let Ok(home) = std::env::var("HOME") {
            paths.push(format!("{}/.local/bin", home));
            paths.push(format!("{}/bin", home));
            // Linux Homebrew
            paths.push(format!("{}/.linuxbrew/bin", home));
        }
    }

    // Add PATH environment variable entries
    if let Ok(path_env) = std::env::var("PATH") {
        let separator = if cfg!(target_os = "windows") {
            ';'
        } else {
            ':'
        };
        for p in path_env.split(separator) {
            if !p.is_empty() && !paths.contains(&p.to_string()) {
                paths.push(p.to_string());
            }
        }
    }

    paths
}

/// Find a command in the search paths
pub fn find_command(cmd: &str) -> Option<String> {
    let search_paths = get_search_paths();

    // On Windows, also try with .exe extension
    let extensions: Vec<&str> = if cfg!(target_os = "windows") {
        vec!["", ".exe", ".cmd", ".bat"]
    } else {
        vec![""]
    };

    for dir in search_paths {
        for ext in &extensions {
            let full_path = Path::new(&dir).join(format!("{}{}", cmd, ext));
            if full_path.exists() {
                return Some(full_path.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// Parse disposition flags from ffprobe output
pub fn parse_disposition(
    disposition: &serde_json::Value,
) -> (bool, bool, bool, bool, bool, bool, bool) {
    let get_flag =
        |key: &str| -> bool { disposition.get(key).and_then(|v| v.as_i64()).unwrap_or(0) == 1 };

    (
        get_flag("default"),
        get_flag("forced"),
        get_flag("hearing_impaired"),
        get_flag("visual_impaired"),
        get_flag("comment"),
        get_flag("lyrics"),
        get_flag("karaoke"),
    )
}

/// Parse a single stream from ffprobe JSON output
pub fn parse_stream(stream: &serde_json::Value) -> StreamInfo {
    let index = stream.get("index").and_then(|v| v.as_i64()).unwrap_or(-1) as i32;

    let codec_type = stream
        .get("codec_type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let stream_type = match codec_type {
        "video" => StreamType::Video,
        "audio" => StreamType::Audio,
        "subtitle" => StreamType::Subtitle,
        "attachment" => StreamType::Attachment,
        "data" => StreamType::Data,
        _ => StreamType::Unknown,
    };

    let tags = stream.get("tags");
    let language = tags
        .and_then(|t| t.get("language"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let title = tags
        .and_then(|t| t.get("title"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let disposition = stream
        .get("disposition")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let (
        is_default,
        is_forced,
        is_hearing_impaired,
        is_visual_impaired,
        is_commentary,
        is_lyrics,
        is_karaoke,
    ) = parse_disposition(&disposition);

    // Check title for additional hints about stream type
    let title_lower = title.as_ref().map(|t| t.to_lowercase()).unwrap_or_default();
    let is_hearing_impaired = is_hearing_impaired
        || title_lower.contains("sdh")
        || title_lower.contains("hearing impaired")
        || title_lower.contains("cc");
    let is_commentary = is_commentary || title_lower.contains("commentary");
    let is_forced = is_forced || title_lower.contains("forced");

    // Detect cover art: typically MJPEG/PNG video streams with attached_pic disposition or single frame
    let codec_name = stream
        .get("codec_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let is_attached_pic = disposition
        .get("attached_pic")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        == 1;
    let nb_frames = stream
        .get("nb_frames")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(-1);
    let is_cover_art = stream_type == StreamType::Video
        && (is_attached_pic
            || (["mjpeg", "png", "bmp", "gif", "webp", "jpeg"]
                .contains(&codec_name.to_lowercase().as_str())
                && (nb_frames == 1 || nb_frames == -1)));

    // Get estimated size from tags or bit_rate calculation
    let duration = stream
        .get("duration")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<f64>().ok());
    let bit_rate_val = stream
        .get("bit_rate")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok());
    let tags_size = tags
        .and_then(|t| t.get("NUMBER_OF_BYTES"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| {
            tags.and_then(|t| t.get("NUMBER_OF_BYTES-eng"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
        });
    let estimated_size = tags_size.or_else(|| match (duration, bit_rate_val) {
        (Some(d), Some(br)) => Some((d * br as f64 / 8.0) as u64),
        _ => None,
    });

    // Check if subtitle before moving stream_type
    let subtitle_format = if stream_type == StreamType::Subtitle {
        stream
            .get("codec_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };

    StreamInfo {
        index,
        stream_type,
        codec_name: stream
            .get("codec_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        codec_long_name: stream
            .get("codec_long_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        language,
        title,
        is_default,
        is_forced,
        is_hearing_impaired,
        is_visual_impaired,
        is_commentary,
        is_lyrics,
        is_karaoke,
        is_cover_art,
        width: stream
            .get("width")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        height: stream
            .get("height")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        frame_rate: stream
            .get("r_frame_rate")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        pixel_format: stream
            .get("pix_fmt")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        sample_rate: stream
            .get("sample_rate")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        channels: stream
            .get("channels")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
        channel_layout: stream
            .get("channel_layout")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        bit_rate: stream
            .get("bit_rate")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        subtitle_format,
        estimated_size,
    }
}

/// Get all media streams from a file using ffprobe
///
/// This function now uses the probe_cache module to avoid redundant ffprobe calls
/// when the same file is queried multiple times (e.g., for metadata + streams + bitrate).
pub fn get_media_streams(path: String) -> Result<MediaStreams, String> {
    let file_path = Path::new(&path);

    // Validate path is within allowed directories
    let validated_path = config::validate_path(file_path)?;

    if !validated_path.exists() {
        return Err("File does not exist".to_string());
    }

    let file_size = fs::metadata(&validated_path).map(|m| m.len()).unwrap_or(0);

    // Use cached probe data to avoid redundant ffprobe calls
    let (_, data, was_cached) = probe_cache::get_probe_data(&path)?;
    debug!("get_media_streams: path={}, cached={}", path, was_cached);

    // Extract duration from format section
    let duration = data
        .get("format")
        .and_then(|f| f.get("duration"))
        .and_then(|d| d.as_str())
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let streams: Vec<StreamInfo> = data
        .get("streams")
        .and_then(|s| s.as_array())
        .map(|arr| arr.iter().map(parse_stream).collect())
        .unwrap_or_default();

    let video_count = streams
        .iter()
        .filter(|s| s.stream_type == StreamType::Video && !s.is_cover_art)
        .count();
    let audio_count = streams
        .iter()
        .filter(|s| s.stream_type == StreamType::Audio)
        .count();
    let subtitle_count = streams
        .iter()
        .filter(|s| s.stream_type == StreamType::Subtitle)
        .count();
    let attachment_count = streams
        .iter()
        .filter(|s| s.stream_type == StreamType::Attachment || s.is_cover_art)
        .count();

    Ok(MediaStreams {
        path,
        streams,
        video_count,
        audio_count,
        subtitle_count,
        attachment_count,
        total_size: file_size,
        duration,
    })
}

/// Remove specified streams from a media file using ffmpeg
pub fn remove_streams(
    path: String,
    stream_indices: Vec<i32>,
    overwrite: bool,
) -> Result<StreamRemovalResult, String> {
    let file_path = Path::new(&path);

    // Validate path is within allowed directories
    let validated_path = config::validate_path(file_path)?;

    if !validated_path.exists() {
        return Err("File does not exist".to_string());
    }

    if stream_indices.is_empty() {
        return Err("No streams selected for removal".to_string());
    }

    let ffmpeg_cmd = find_command("ffmpeg").unwrap_or_else(|| "ffmpeg".to_string());

    // Create output path - either temp file for overwrite or _modified suffix
    let stem = validated_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = validated_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv");
    let parent = validated_path.parent().unwrap_or(Path::new("."));

    let (output_path, temp_path) = if overwrite {
        // Use a temp file, then replace original
        let temp = parent.join(format!("{}_temp_{}.{}", stem, std::process::id(), ext));
        (validated_path.to_path_buf(), temp)
    } else {
        let modified = parent.join(format!("{}_modified.{}", stem, ext));
        (modified.clone(), modified)
    };

    // First, get total stream count
    let streams_result = get_media_streams(path.clone())?;
    let total_streams = streams_result.streams.len();

    // Build ffmpeg arguments
    let mut args: Vec<String> = vec![
        "-i".to_string(),
        path.clone(),
        "-map".to_string(),
        "0".to_string(), // Start by mapping all streams
    ];

    // Add negative mappings for streams to remove
    for idx in &stream_indices {
        if *idx >= 0 && (*idx as usize) < total_streams {
            args.push("-map".to_string());
            args.push(format!("-0:{}", idx));
        }
    }

    // Copy all streams without re-encoding
    args.extend([
        "-c".to_string(),
        "copy".to_string(),
        "-y".to_string(), // Overwrite output file if exists
        temp_path.to_string_lossy().to_string(),
    ]);

    let output = Command::new(&ffmpeg_cmd)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if !output.status.success() {
        // Clean up temp file on failure
        if overwrite {
            let _ = fs::remove_file(&temp_path);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffmpeg failed: {}", stderr));
    }

    // If overwriting, replace original with temp file
    if overwrite {
        // Verify temp file exists and has reasonable size before replacing original
        let temp_metadata = fs::metadata(&temp_path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to verify temp file: {}", e)
        })?;

        let temp_size = temp_metadata.len();
        if temp_size == 0 {
            let _ = fs::remove_file(&temp_path);
            return Err("Temp file is empty - aborting to prevent data loss".to_string());
        }

        // Get original file size for comparison
        let original_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        // Warn if temp file is suspiciously small compared to original (more than 90% smaller)
        // This could indicate an incomplete write or encoding failure
        if original_size > 0 && temp_size < original_size / 10 {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "Temp file ({} bytes) is suspiciously smaller than original ({} bytes) - aborting to prevent data loss",
                temp_size, original_size
            ));
        }

        // All checks passed, safe to replace
        fs::remove_file(&path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to remove original file: {}", e)
        })?;
        fs::rename(&temp_path, &path).map_err(|e| format!("Failed to rename temp file: {}", e))?;

        // Invalidate probe cache since the file has been modified
        probe_cache::invalidate_cache(&path);
    }

    let final_message = if overwrite {
        format!(
            "Successfully removed {} stream(s). Original file updated.",
            stream_indices.len()
        )
    } else {
        format!(
            "Successfully removed {} stream(s). Output saved to: {}",
            stream_indices.len(),
            output_path.display()
        )
    };

    // Also invalidate cache for the output path if it's different from the original
    if !overwrite {
        probe_cache::invalidate_cache(&output_path.to_string_lossy());
    }

    Ok(StreamRemovalResult {
        success: true,
        output_path: output_path.to_string_lossy().to_string(),
        message: final_message,
    })
}
