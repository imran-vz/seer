use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::{TitleBarStyle, WebviewUrl, WebviewWindowBuilder};
use trash;

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mp3", "flac", "wav", "aac", "ogg",
    "wma", "m4a", "opus", "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "heic",
];

#[derive(Serialize)]
pub struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
    is_media: bool,
    size: u64,
    modified: Option<String>,
}

#[derive(Serialize)]
pub struct FileMetadata {
    path: String,
    name: String,
    size: u64,
    modified: Option<String>,
    created: Option<String>,
    is_media: bool,
    extension: Option<String>,
    ffprobe_data: Option<String>,
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

#[derive(Debug, Serialize)]
pub struct MediaStreams {
    pub path: String,
    pub streams: Vec<StreamInfo>,
    pub video_count: usize,
    pub audio_count: usize,
    pub subtitle_count: usize,
    pub attachment_count: usize,
    pub total_size: u64,
}

#[derive(Debug, Serialize)]
pub struct StreamRemovalResult {
    pub success: bool,
    pub output_path: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct FileOperationResult {
    pub success: bool,
    pub message: String,
    pub new_path: Option<String>,
}

fn is_media_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| MEDIA_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn format_time(time: std::io::Result<std::time::SystemTime>) -> Option<String> {
    time.ok().map(|t| {
        let datetime: chrono::DateTime<chrono::Local> = t.into();
        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    })
}

fn get_search_paths() -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();

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

fn find_command(cmd: &str) -> Option<String> {
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

fn parse_disposition(
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

fn parse_stream(stream: &serde_json::Value) -> StreamInfo {
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

#[tauri::command]
fn get_media_streams(path: String) -> Result<MediaStreams, String> {
    let file_path = Path::new(&path);
    if !file_path.exists() {
        return Err("File does not exist".to_string());
    }

    let ffprobe_cmd = find_command("ffprobe").unwrap_or_else(|| "ffprobe".to_string());

    let file_size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);

    let output = Command::new(&ffprobe_cmd)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
            &path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json_str =
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 output: {}", e))?;

    let data: serde_json::Value =
        serde_json::from_str(&json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;

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
    })
}

#[tauri::command]
fn remove_streams(
    path: String,
    stream_indices: Vec<i32>,
    overwrite: bool,
) -> Result<StreamRemovalResult, String> {
    let file_path = Path::new(&path);
    if !file_path.exists() {
        return Err("File does not exist".to_string());
    }

    if stream_indices.is_empty() {
        return Err("No streams selected for removal".to_string());
    }

    let ffmpeg_cmd = find_command("ffmpeg").unwrap_or_else(|| "ffmpeg".to_string());

    // Create output path - either temp file for overwrite or _modified suffix
    let stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mkv");
    let parent = file_path.parent().unwrap_or(Path::new("."));

    let (output_path, temp_path) = if overwrite {
        // Use a temp file, then replace original
        let temp = parent.join(format!("{}_temp_{}.{}", stem, std::process::id(), ext));
        (file_path.to_path_buf(), temp)
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
        fs::remove_file(&path).map_err(|e| {
            let _ = fs::remove_file(&temp_path);
            format!("Failed to remove original file: {}", e)
        })?;
        fs::rename(&temp_path, &path).map_err(|e| format!("Failed to rename temp file: {}", e))?;
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

    Ok(StreamRemovalResult {
        success: true,
        output_path: output_path.to_string_lossy().to_string(),
        message: final_message,
    })
}

#[tauri::command]
fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    let dir_path = if path.is_empty() {
        dirs::home_dir().unwrap_or_else(|| Path::new("/").to_path_buf())
    } else {
        Path::new(&path).to_path_buf()
    };

    let entries = fs::read_dir(&dir_path).map_err(|e| e.to_string())?;
    let mut files: Vec<FileEntry> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            continue;
        }

        let metadata = entry.metadata().ok();
        let is_dir = path.is_dir();
        let is_media = !is_dir && is_media_file(&path);

        files.push(FileEntry {
            name,
            path: path.to_string_lossy().to_string(),
            is_dir,
            is_media,
            size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
            modified: metadata.and_then(|m| format_time(m.modified())),
        });
    }

    files.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(files)
}

#[tauri::command]
fn get_file_metadata(path: String) -> Result<FileMetadata, String> {
    let file_path = Path::new(&path);
    let metadata = fs::metadata(&file_path).map_err(|e| e.to_string())?;

    let is_media = is_media_file(file_path);

    let ffprobe_data = if is_media {
        let ffprobe_cmd = find_command("ffprobe").unwrap_or_else(|| "ffprobe".to_string());
        Command::new(ffprobe_cmd)
            .args([
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                &path,
            ])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
    } else {
        None
    };

    Ok(FileMetadata {
        path: path.clone(),
        name: file_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        size: metadata.len(),
        modified: format_time(metadata.modified()),
        created: format_time(metadata.created()),
        is_media,
        extension: file_path
            .extension()
            .map(|e| e.to_string_lossy().to_string()),
        ffprobe_data,
    })
}

#[tauri::command]
fn get_home_dir() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string())
}

#[derive(Serialize, Deserialize)]
pub struct DependencyStatus {
    name: String,
    installed: bool,
    version: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DependenciesResult {
    all_installed: bool,
    dependencies: Vec<DependencyStatus>,
    platform: String,
}

fn check_command(cmd: &str, version_args: &[&str]) -> DependencyStatus {
    let cmd_path = find_command(cmd);

    let output = match &cmd_path {
        Some(path) => Command::new(path).args(version_args).output(),
        None => Command::new(cmd).args(version_args).output(),
    };

    match output {
        Ok(result) if result.status.success() => {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            let output_str = if stdout.is_empty() { stderr } else { stdout };
            let version = output_str.lines().next().map(|s| s.trim().to_string());
            DependencyStatus {
                name: cmd.to_string(),
                installed: true,
                version,
            }
        }
        _ => DependencyStatus {
            name: cmd.to_string(),
            installed: false,
            version: None,
        },
    }
}

#[tauri::command]
fn rename_file(path: String, new_name: String) -> Result<FileOperationResult, String> {
    let file_path = Path::new(&path);
    if !file_path.exists() {
        return Err("File does not exist".to_string());
    }

    let parent = file_path.parent().ok_or("Cannot get parent directory")?;
    let new_path = parent.join(&new_name);

    if new_path.exists() {
        return Err(format!("A file named '{}' already exists", new_name));
    }

    fs::rename(&file_path, &new_path).map_err(|e| format!("Failed to rename: {}", e))?;

    Ok(FileOperationResult {
        success: true,
        message: format!("Renamed to '{}'", new_name),
        new_path: Some(new_path.to_string_lossy().to_string()),
    })
}

#[tauri::command]
fn delete_file(path: String, permanent: bool) -> Result<FileOperationResult, String> {
    let file_path = Path::new(&path);
    if !file_path.exists() {
        return Err("File does not exist".to_string());
    }

    if permanent {
        // Permanent delete
        if file_path.is_dir() {
            fs::remove_dir_all(&file_path)
                .map_err(|e| format!("Failed to delete folder: {}", e))?;
        } else {
            fs::remove_file(&file_path).map_err(|e| format!("Failed to delete file: {}", e))?;
        }
        Ok(FileOperationResult {
            success: true,
            message: "Permanently deleted".to_string(),
            new_path: None,
        })
    } else {
        // Move to trash (works on Windows, macOS, and Linux)
        trash::delete(&file_path).map_err(|e| format!("Failed to move to trash: {}", e))?;
        Ok(FileOperationResult {
            success: true,
            message: "Moved to trash".to_string(),
            new_path: None,
        })
    }
}

#[tauri::command]
fn move_file(path: String, destination: String) -> Result<FileOperationResult, String> {
    let file_path = Path::new(&path);
    let dest_path = Path::new(&destination);

    if !file_path.exists() {
        return Err("Source file does not exist".to_string());
    }

    if !dest_path.is_dir() {
        return Err("Destination must be a directory".to_string());
    }

    let file_name = file_path.file_name().ok_or("Cannot get file name")?;
    let new_path = dest_path.join(file_name);

    if new_path.exists() {
        return Err(format!(
            "A file named '{}' already exists in destination",
            file_name.to_string_lossy()
        ));
    }

    fs::rename(&file_path, &new_path).map_err(|e| format!("Failed to move: {}", e))?;

    Ok(FileOperationResult {
        success: true,
        message: format!("Moved to '{}'", destination),
        new_path: Some(new_path.to_string_lossy().to_string()),
    })
}

#[tauri::command]
fn copy_file(path: String, destination: String) -> Result<FileOperationResult, String> {
    let file_path = Path::new(&path);
    let dest_path = Path::new(&destination);

    if !file_path.exists() {
        return Err("Source file does not exist".to_string());
    }

    if !dest_path.is_dir() {
        return Err("Destination must be a directory".to_string());
    }

    let file_name = file_path.file_name().ok_or("Cannot get file name")?;
    let new_path = dest_path.join(file_name);

    if new_path.exists() {
        return Err(format!(
            "A file named '{}' already exists in destination",
            file_name.to_string_lossy()
        ));
    }

    if file_path.is_dir() {
        copy_dir_recursive(&file_path, &new_path)
            .map_err(|e| format!("Failed to copy folder: {}", e))?;
    } else {
        fs::copy(&file_path, &new_path).map_err(|e| format!("Failed to copy: {}", e))?;
    }

    Ok(FileOperationResult {
        success: true,
        message: format!("Copied to '{}'", destination),
        new_path: Some(new_path.to_string_lossy().to_string()),
    })
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else {
            fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}

#[tauri::command]
fn create_folder(path: String, name: String) -> Result<FileOperationResult, String> {
    let parent_path = Path::new(&path);
    if !parent_path.is_dir() {
        return Err("Parent path must be a directory".to_string());
    }

    let new_folder = parent_path.join(&name);
    if new_folder.exists() {
        return Err(format!("A folder named '{}' already exists", name));
    }

    fs::create_dir(&new_folder).map_err(|e| format!("Failed to create folder: {}", e))?;

    Ok(FileOperationResult {
        success: true,
        message: format!("Created folder '{}'", name),
        new_path: Some(new_folder.to_string_lossy().to_string()),
    })
}

#[tauri::command]
fn reveal_in_folder(path: String) -> Result<FileOperationResult, String> {
    let file_path = Path::new(&path);
    if !file_path.exists() {
        return Err("Path does not exist".to_string());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| format!("Failed to reveal in Finder: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| format!("Failed to reveal in Explorer: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Try different file managers
        let parent = file_path.parent().unwrap_or(Path::new("/"));
        Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| format!("Failed to open file manager: {}", e))?;
    }

    Ok(FileOperationResult {
        success: true,
        message: "Revealed in file manager".to_string(),
        new_path: None,
    })
}

#[tauri::command]
fn check_dependencies() -> DependenciesResult {
    let ffprobe = check_command("ffprobe", &["-version"]);
    let ffmpeg = check_command("ffmpeg", &["-version"]);
    let exiftool = check_command("exiftool", &["-ver"]);

    let all_installed = ffprobe.installed && ffmpeg.installed && exiftool.installed;

    let platform = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
    .to_string();

    DependenciesResult {
        all_installed,
        dependencies: vec![ffprobe, ffmpeg, exiftool],
        platform,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Seer")
                .inner_size(1200.0, 800.0)
                .hidden_title(true)
                .decorations(true);

            // set transparent title bar only when building for macOS
            #[cfg(target_os = "macos")]
            let win_builder = win_builder
                .title_bar_style(TitleBarStyle::Overlay)
                .traffic_light_position(tauri::Position::Logical(tauri::LogicalPosition {
                    x: 20.0,
                    y: 24.0,
                }));

            let window = win_builder.build().unwrap();

            // set background color only when building for macOS
            #[cfg(target_os = "macos")]
            {
                use objc2::rc::Retained;
                use objc2_app_kit::{NSColor, NSWindow};

                let ns_window: *mut std::ffi::c_void = window.ns_window().unwrap();
                let ns_window: Retained<NSWindow> =
                    unsafe { Retained::retain(ns_window as *mut NSWindow).unwrap() };

                // Dark background color (matches dark theme)
                let bg_color = NSColor::colorWithRed_green_blue_alpha(
                    10.0 / 255.0,
                    10.0 / 255.0,
                    10.0 / 255.0,
                    1.0,
                );
                ns_window.setBackgroundColor(Some(&bg_color));
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_directory,
            get_file_metadata,
            get_home_dir,
            check_dependencies,
            get_media_streams,
            remove_streams,
            rename_file,
            delete_file,
            move_file,
            copy_file,
            create_folder,
            reveal_in_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
