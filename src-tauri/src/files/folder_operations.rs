//! Folder creation operations
//!
//! This module handles creating folders from file selections with multiple modes.

use crate::media::find_command;
use crate::types::{FolderCreationMode, FolderCreationResult, GroupCriteria};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::UNIX_EPOCH;

/// Create folders per file (movie.mp4 → movie/movie.mp4)
fn create_folder_per_file(paths: Vec<String>, parent: &str) -> Result<FolderCreationResult, String> {
    let parent_path = Path::new(parent);
    let mut success = 0;
    let mut failed = 0;
    let mut folders_created = Vec::new();
    let mut errors = Vec::new();

    for path in paths {
        let source = Path::new(&path);
        let file_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid file name: {}", path))?;

        // Get base name without extension
        let base_name = source
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(file_name);

        // Create folder path
        let folder_path = parent_path.join(base_name);

        match fs::create_dir_all(&folder_path) {
            Ok(_) => {
                folders_created.push(folder_path.to_string_lossy().to_string());

                // Move file into folder
                let dest = folder_path.join(file_name);
                match fs::rename(&source, &dest) {
                    Ok(_) => success += 1,
                    Err(e) => {
                        failed += 1;
                        errors.push(format!("{}: {}", file_name, e));
                    }
                }
            }
            Err(e) => {
                failed += 1;
                errors.push(format!("Failed to create folder for {}: {}", file_name, e));
            }
        }
    }

    Ok(FolderCreationResult {
        success,
        failed,
        folders_created,
        errors,
    })
}

/// Create single folder and move all files into it
fn create_single_folder_and_move(
    paths: Vec<String>,
    folder_name: String,
    parent: &str,
) -> Result<FolderCreationResult, String> {
    let parent_path = Path::new(parent);
    let folder_path = parent_path.join(&folder_name);

    // Create the folder
    fs::create_dir_all(&folder_path)
        .map_err(|e| format!("Failed to create folder '{}': {}", folder_name, e))?;

    let mut success = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    // Move all files into the folder
    for path in paths {
        let source = Path::new(&path);
        let file_name = source
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| format!("Invalid file name: {}", path))?;

        let dest = folder_path.join(file_name);
        match fs::rename(&source, &dest) {
            Ok(_) => success += 1,
            Err(e) => {
                failed += 1;
                errors.push(format!("{}: {}", file_name, e));
            }
        }
    }

    Ok(FolderCreationResult {
        success,
        failed,
        folders_created: vec![folder_path.to_string_lossy().to_string()],
        errors,
    })
}

/// Group files by criteria and create folders
fn create_grouped_folders(
    paths: Vec<String>,
    criteria: GroupCriteria,
    parent: &str,
) -> Result<FolderCreationResult, String> {
    let parent_path = Path::new(parent);

    // Group files by criteria
    let groups = match criteria {
        GroupCriteria::Extension => group_by_extension(&paths)?,
        GroupCriteria::DateModified { granularity } => {
            group_by_date(&paths, granularity.as_str())?
        }
        GroupCriteria::MediaType => group_by_media_type(&paths)?,
        GroupCriteria::Resolution => group_by_resolution(&paths)?,
        GroupCriteria::Codec => group_by_codec(&paths)?,
    };

    let mut success = 0;
    let mut failed = 0;
    let mut folders_created = Vec::new();
    let mut errors = Vec::new();

    // Create folders and move files
    for (folder_name, file_paths) in groups {
        let folder_path = parent_path.join(&folder_name);

        match fs::create_dir_all(&folder_path) {
            Ok(_) => {
                folders_created.push(folder_path.to_string_lossy().to_string());

                for path in file_paths {
                    let source = Path::new(&path);
                    let file_name = source.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    let dest = folder_path.join(file_name);
                    match fs::rename(&source, &dest) {
                        Ok(_) => success += 1,
                        Err(e) => {
                            failed += 1;
                            errors.push(format!("{}: {}", file_name, e));
                        }
                    }
                }
            }
            Err(e) => {
                failed += file_paths.len();
                errors.push(format!("Failed to create folder '{}': {}", folder_name, e));
            }
        }
    }

    Ok(FolderCreationResult {
        success,
        failed,
        folders_created,
        errors,
    })
}

/// Group files by extension
fn group_by_extension(paths: &[String]) -> Result<HashMap<String, Vec<String>>, String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in paths {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("no_extension")
            .to_lowercase();

        groups.entry(ext).or_insert_with(Vec::new).push(path.clone());
    }

    Ok(groups)
}

/// Group files by date modified
fn group_by_date(paths: &[String], granularity: &str) -> Result<HashMap<String, Vec<String>>, String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in paths {
        let metadata = fs::metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {}", path, e))?;

        let modified = metadata
            .modified()
            .map_err(|e| format!("Failed to read modified time for {}: {}", path, e))?;

        // Convert SystemTime to seconds since epoch
        let duration = modified
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Failed to convert system time: {}", e))?;

        // Format using chrono DateTime
        use chrono::{DateTime, Utc};
        let datetime = DateTime::<Utc>::from_timestamp(duration.as_secs() as i64, 0)
            .ok_or_else(|| "Invalid timestamp".to_string())?;

        let folder_name = match granularity {
            "day" => datetime.format("%Y-%m-%d").to_string(),
            "month" => datetime.format("%Y-%m").to_string(),
            "year" => datetime.format("%Y").to_string(),
            _ => datetime.format("%Y-%m-%d").to_string(),
        };

        groups
            .entry(folder_name)
            .or_insert_with(Vec::new)
            .push(path.clone());
    }

    Ok(groups)
}

/// Group files by media type (video/audio)
fn group_by_media_type(paths: &[String]) -> Result<HashMap<String, Vec<String>>, String> {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in paths {
        let ext = Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        let media_type = match ext.as_deref() {
            Some("mp4") | Some("mkv") | Some("avi") | Some("mov") | Some("webm") | Some("flv")
            | Some("wmv") | Some("m4v") => "video",
            Some("mp3") | Some("flac") | Some("wav") | Some("aac") | Some("ogg") | Some("m4a")
            | Some("wma") => "audio",
            _ => "other",
        };

        groups
            .entry(media_type.to_string())
            .or_insert_with(Vec::new)
            .push(path.clone());
    }

    Ok(groups)
}

/// Group files by resolution (requires ffprobe)
fn group_by_resolution(paths: &[String]) -> Result<HashMap<String, Vec<String>>, String> {
    let ffprobe = find_command("ffprobe")
        .ok_or_else(|| "ffprobe not found. Required for resolution grouping.".to_string())?;

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in paths {
        let resolution = get_resolution(path, &ffprobe).unwrap_or_else(|_| "unknown".to_string());

        groups
            .entry(resolution)
            .or_insert_with(Vec::new)
            .push(path.clone());
    }

    Ok(groups)
}

/// Group files by codec (requires ffprobe)
fn group_by_codec(paths: &[String]) -> Result<HashMap<String, Vec<String>>, String> {
    let ffprobe = find_command("ffprobe")
        .ok_or_else(|| "ffprobe not found. Required for codec grouping.".to_string())?;

    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for path in paths {
        let codec = get_video_codec(path, &ffprobe).unwrap_or_else(|_| "unknown".to_string());

        groups
            .entry(codec)
            .or_insert_with(Vec::new)
            .push(path.clone());
    }

    Ok(groups)
}

/// Get video resolution using ffprobe
fn get_resolution(path: &str, ffprobe: &String) -> Result<String, String> {
    let output = Command::new(ffprobe)
        .args(&[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=s=x:p=0",
            path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let resolution = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if resolution.is_empty() {
        return Ok("unknown".to_string());
    }

    // Parse width and height
    let parts: Vec<&str> = resolution.split('x').collect();
    if parts.len() == 2 {
        if let (Ok(width), Ok(height)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
            // Map to common names
            return Ok(match (width, height) {
                (3840, 2160) => "4K".to_string(),
                (2560, 1440) => "1440p".to_string(),
                (1920, 1080) => "1080p".to_string(),
                (1280, 720) => "720p".to_string(),
                (854, 480) => "480p".to_string(),
                _ => resolution,
            });
        }
    }

    Ok(resolution)
}

/// Get video codec using ffprobe
fn get_video_codec(path: &str, ffprobe: &String) -> Result<String, String> {
    let output = Command::new(ffprobe)
        .args(&[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let codec = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if codec.is_empty() {
        return Ok("unknown".to_string());
    }

    Ok(codec)
}

/// Main entry point for folder creation
pub fn create_folders_from_selection(
    paths: Vec<String>,
    mode: FolderCreationMode,
    parent_dir: String,
) -> Result<FolderCreationResult, String> {
    match mode {
        FolderCreationMode::PerFile => create_folder_per_file(paths, &parent_dir),
        FolderCreationMode::Grouped { criteria } => {
            create_grouped_folders(paths, criteria, &parent_dir)
        }
        FolderCreationMode::Single { name } => {
            create_single_folder_and_move(paths, name, &parent_dir)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== group_by_extension tests ==========

    #[test]
    fn test_group_by_extension_empty() {
        let paths: Vec<String> = vec![];
        let result = group_by_extension(&paths).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_group_by_extension_single() {
        let paths = vec!["/path/file.mp4".to_string()];
        let result = group_by_extension(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["mp4"].len(), 1);
        assert_eq!(result["mp4"][0], "/path/file.mp4");
    }

    #[test]
    fn test_group_by_extension_multiple_same() {
        let paths = vec![
            "/path/file1.mp4".to_string(),
            "/path/file2.mp4".to_string(),
            "/path/file3.mp4".to_string(),
        ];
        let result = group_by_extension(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["mp4"].len(), 3);
    }

    #[test]
    fn test_group_by_extension_multiple_different() {
        let paths = vec![
            "/path/file1.mp4".to_string(),
            "/path/file2.mkv".to_string(),
            "/path/file3.avi".to_string(),
        ];
        let result = group_by_extension(&paths).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result["mp4"].len(), 1);
        assert_eq!(result["mkv"].len(), 1);
        assert_eq!(result["avi"].len(), 1);
    }

    #[test]
    fn test_group_by_extension_case_insensitive() {
        let paths = vec![
            "/path/file1.MP4".to_string(),
            "/path/file2.mp4".to_string(),
            "/path/file3.Mp4".to_string(),
        ];
        let result = group_by_extension(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["mp4"].len(), 3);
    }

    #[test]
    fn test_group_by_extension_no_extension() {
        let paths = vec!["/path/file".to_string()];
        let result = group_by_extension(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["no_extension"].len(), 1);
    }

    #[test]
    fn test_group_by_extension_mixed() {
        let paths = vec![
            "/path/file1.mp4".to_string(),
            "/path/file2.mkv".to_string(),
            "/path/file3.mp4".to_string(),
            "/path/file4".to_string(),
        ];
        let result = group_by_extension(&paths).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result["mp4"].len(), 2);
        assert_eq!(result["mkv"].len(), 1);
        assert_eq!(result["no_extension"].len(), 1);
    }

    // ========== group_by_media_type tests ==========

    #[test]
    fn test_group_by_media_type_empty() {
        let paths: Vec<String> = vec![];
        let result = group_by_media_type(&paths).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_group_by_media_type_video() {
        let paths = vec![
            "/path/file1.mp4".to_string(),
            "/path/file2.mkv".to_string(),
            "/path/file3.avi".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["video"].len(), 3);
    }

    #[test]
    fn test_group_by_media_type_audio() {
        let paths = vec![
            "/path/file1.mp3".to_string(),
            "/path/file2.flac".to_string(),
            "/path/file3.wav".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["audio"].len(), 3);
    }

    #[test]
    fn test_group_by_media_type_mixed() {
        let paths = vec![
            "/path/video.mp4".to_string(),
            "/path/audio.mp3".to_string(),
            "/path/video2.mkv".to_string(),
            "/path/audio2.flac".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result["video"].len(), 2);
        assert_eq!(result["audio"].len(), 2);
    }

    #[test]
    fn test_group_by_media_type_other() {
        let paths = vec![
            "/path/doc.pdf".to_string(),
            "/path/img.jpg".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["other"].len(), 2);
    }

    #[test]
    fn test_group_by_media_type_case_insensitive() {
        let paths = vec![
            "/path/file1.MP4".to_string(),
            "/path/file2.Mp3".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result["video"].len(), 1);
        assert_eq!(result["audio"].len(), 1);
    }

    #[test]
    fn test_group_by_media_type_all_types() {
        let paths = vec![
            "/path/video.mp4".to_string(),
            "/path/audio.mp3".to_string(),
            "/path/doc.txt".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result["video"].len(), 1);
        assert_eq!(result["audio"].len(), 1);
        assert_eq!(result["other"].len(), 1);
    }

    #[test]
    fn test_group_by_media_type_all_video_formats() {
        let paths = vec![
            "/path/1.mp4".to_string(),
            "/path/2.mkv".to_string(),
            "/path/3.avi".to_string(),
            "/path/4.mov".to_string(),
            "/path/5.webm".to_string(),
            "/path/6.flv".to_string(),
            "/path/7.wmv".to_string(),
            "/path/8.m4v".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["video"].len(), 8);
    }

    #[test]
    fn test_group_by_media_type_all_audio_formats() {
        let paths = vec![
            "/path/1.mp3".to_string(),
            "/path/2.flac".to_string(),
            "/path/3.wav".to_string(),
            "/path/4.aac".to_string(),
            "/path/5.ogg".to_string(),
            "/path/6.m4a".to_string(),
            "/path/7.wma".to_string(),
        ];
        let result = group_by_media_type(&paths).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["audio"].len(), 7);
    }
}
