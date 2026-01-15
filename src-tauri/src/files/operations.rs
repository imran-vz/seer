//! File system operations
//!
//! This module handles:
//! - Directory listing
//! - File metadata retrieval
//! - File operations (rename, delete, move, copy)
//! - Dependency checking

use log::{debug, info, warn};
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::config;
use crate::media::{find_command, get_probe_string};
use crate::types::{
    is_video_audio_extension, DependenciesResult, DependencyStatus, FileEntry, FileMetadata,
    FileOperationResult, MEDIA_EXTENSIONS,
};

/// Check if a file is a media file based on extension
pub fn is_media_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| MEDIA_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Format a system time as a human-readable string
pub fn format_time(time: std::io::Result<std::time::SystemTime>) -> Option<String> {
    time.ok().map(|t| {
        let datetime: chrono::DateTime<chrono::Local> = t.into();
        datetime.format("%Y-%m-%d %H:%M:%S").to_string()
    })
}

/// List directory contents
pub fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    debug!("list_directory called: path={:?}", path);

    let dir_path = if path.is_empty() {
        let home = dirs::home_dir().unwrap_or_else(|| Path::new("/").to_path_buf());
        debug!("Using home directory: {:?}", home);
        home
    } else {
        Path::new(&path).to_path_buf()
    };

    // Validate path is within allowed directories
    let validated_path = config::validate_path(&dir_path)?;
    debug!("Validated path: {:?}", validated_path);

    let entries = fs::read_dir(&validated_path).map_err(|e| {
        warn!("Failed to read directory {:?}: {}", validated_path, e);
        e.to_string()
    })?;

    let mut files: Vec<FileEntry> = Vec::new();
    let mut hidden_count = 0;
    let mut media_count = 0;
    let mut dir_count = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        if name.starts_with('.') {
            hidden_count += 1;
            continue;
        }

        let metadata = entry.metadata().ok();
        let is_dir = path.is_dir();
        let is_media = !is_dir && is_media_file(&path);

        if is_dir {
            dir_count += 1;
        } else if is_media {
            media_count += 1;
        }

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

    info!(
        "Listed directory {:?}: {} entries ({} dirs, {} media, {} hidden)",
        validated_path,
        files.len(),
        dir_count,
        media_count,
        hidden_count
    );

    Ok(files)
}

/// Get file metadata including ffprobe data for media files
pub fn get_file_metadata(path: String) -> Result<FileMetadata, String> {
    debug!("get_file_metadata called: path={:?}", path);

    let file_path = Path::new(&path);

    // Validate path is within allowed directories
    let validated_path = config::validate_path(file_path)?;

    let metadata = fs::metadata(&validated_path).map_err(|e| {
        warn!("Failed to get metadata for {:?}: {}", validated_path, e);
        e.to_string()
    })?;

    let is_media = is_media_file(file_path);
    debug!("File {:?} is_media={}", validated_path, is_media);

    // Only fetch ffprobe data for video/audio files, not images
    let extension = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_video_audio = is_video_audio_extension(extension);

    let ffprobe_data = if is_media && is_video_audio {
        debug!(
            "Getting ffprobe data for video/audio file: {:?}",
            validated_path
        );

        // Use cached probe data to avoid redundant ffprobe calls
        match get_probe_string(&path) {
            Ok(json_str) => {
                debug!("ffprobe data retrieved for {:?}", path);
                Some(json_str)
            }
            Err(e) => {
                warn!("Failed to get ffprobe data for {:?}: {}", path, e);
                None
            }
        }
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

/// Get home directory path
pub fn get_home_dir() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string())
}

/// Check if a command is installed and get its version
pub fn check_command(cmd: &str, version_args: &[&str]) -> DependencyStatus {
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

/// Check all required dependencies
pub fn check_dependencies() -> DependenciesResult {
    let ffmpeg = check_command("ffmpeg", &["-version"]);
    let ffprobe = check_command("ffprobe", &["-version"]);

    let all_installed = ffmpeg.installed && ffprobe.installed;

    DependenciesResult {
        all_installed,
        dependencies: vec![ffmpeg, ffprobe],
        platform: std::env::consts::OS.to_string(),
    }
}

/// Rename a file
pub fn rename_file(path: String, new_name: String) -> Result<FileOperationResult, String> {
    info!("rename_file: from={:?} to={:?}", path, new_name);

    let file_path = Path::new(&path);

    // Validate source path is within allowed directories
    let validated_path = config::validate_path(file_path)?;

    if !validated_path.exists() {
        warn!("Rename failed: file does not exist: {:?}", validated_path);
        return Err("File does not exist".to_string());
    }

    let parent = validated_path
        .parent()
        .ok_or("Cannot get parent directory")?;
    let new_path = parent.join(&new_name);
    debug!("Rename destination: {:?}", new_path);

    // Validate destination path is also within allowed directories
    config::validate_path(&new_path)?;

    if new_path.exists() {
        warn!("Rename failed: destination already exists: {:?}", new_path);
        return Err(format!("A file named '{}' already exists", new_name));
    }

    fs::rename(&validated_path, &new_path).map_err(|e| {
        warn!("Rename failed: {:?}", e);
        format!("Failed to rename: {}", e)
    })?;

    info!(
        "Successfully renamed {:?} to {:?}",
        validated_path, new_path
    );

    Ok(FileOperationResult {
        success: true,
        message: format!("Renamed to '{}'", new_name),
        new_path: Some(new_path.to_string_lossy().to_string()),
    })
}

/// Delete a file (to trash or permanently)
pub fn delete_file(path: String, permanent: bool) -> Result<FileOperationResult, String> {
    info!("delete_file: path={:?}, permanent={}", path, permanent);

    let file_path = Path::new(&path);

    // Validate path is within allowed directories
    let validated_path = config::validate_path(file_path)?;

    if !validated_path.exists() {
        warn!("Delete failed: file does not exist: {:?}", validated_path);
        return Err("File does not exist".to_string());
    }

    if permanent {
        // Permanent delete
        if validated_path.is_dir() {
            debug!("Permanently deleting directory: {:?}", validated_path);
            fs::remove_dir_all(&validated_path).map_err(|e| {
                warn!("Failed to delete folder {:?}: {}", validated_path, e);
                format!("Failed to delete folder: {}", e)
            })?;
        } else {
            debug!("Permanently deleting file: {:?}", validated_path);
            fs::remove_file(&validated_path).map_err(|e| {
                warn!("Failed to delete file {:?}: {}", validated_path, e);
                format!("Failed to delete file: {}", e)
            })?;
        }
        info!("Successfully permanently deleted: {:?}", validated_path);
        Ok(FileOperationResult {
            success: true,
            message: "Permanently deleted".to_string(),
            new_path: None,
        })
    } else {
        // Move to trash (works on Windows, macOS, and Linux)
        debug!("Moving to trash: {:?}", validated_path);
        trash::delete(&validated_path).map_err(|e| {
            warn!("Failed to move {:?} to trash: {}", validated_path, e);
            format!("Failed to move to trash: {}", e)
        })?;
        info!("Successfully moved to trash: {:?}", validated_path);
        Ok(FileOperationResult {
            success: true,
            message: "Moved to trash".to_string(),
            new_path: None,
        })
    }
}

/// Move a file to a new location
pub fn move_file(path: String, destination: String) -> Result<FileOperationResult, String> {
    info!("move_file: from={:?} to={:?}", path, destination);

    let file_path = Path::new(&path);
    let dest_path = Path::new(&destination);

    // Validate both source and destination paths
    let validated_src = config::validate_path(file_path)?;
    let validated_dest = config::validate_path(dest_path)?;

    if !validated_src.exists() {
        warn!("Move failed: source does not exist: {:?}", validated_src);
        return Err("Source file does not exist".to_string());
    }

    if !validated_dest.is_dir() {
        warn!(
            "Move failed: destination is not a directory: {:?}",
            validated_dest
        );
        return Err("Destination must be a directory".to_string());
    }

    let file_name = validated_src.file_name().ok_or("Cannot get file name")?;
    let new_path = validated_dest.join(file_name);
    debug!("Move destination path: {:?}", new_path);

    // Validate final destination path
    config::validate_path(&new_path)?;

    if new_path.exists() {
        warn!(
            "Move failed: destination file already exists: {:?}",
            new_path
        );
        return Err(format!(
            "A file named '{}' already exists in destination",
            file_name.to_string_lossy()
        ));
    }

    fs::rename(&validated_src, &new_path).map_err(|e| {
        warn!("Move failed: {:?}", e);
        format!("Failed to move: {}", e)
    })?;

    info!("Successfully moved {:?} to {:?}", validated_src, new_path);

    Ok(FileOperationResult {
        success: true,
        message: format!("Moved to '{}'", destination),
        new_path: Some(new_path.to_string_lossy().to_string()),
    })
}

/// Copy a file to a new location
pub fn copy_file(path: String, destination: String) -> Result<FileOperationResult, String> {
    info!("copy_file: from={:?} to={:?}", path, destination);

    let file_path = Path::new(&path);
    let dest_path = Path::new(&destination);

    // Validate both source and destination paths
    let validated_src = config::validate_path(file_path)?;
    let validated_dest = config::validate_path(dest_path)?;

    if !validated_src.exists() {
        warn!("Copy failed: source does not exist: {:?}", validated_src);
        return Err("Source file does not exist".to_string());
    }

    if !validated_dest.is_dir() {
        warn!(
            "Copy failed: destination is not a directory: {:?}",
            validated_dest
        );
        return Err("Destination must be a directory".to_string());
    }

    let file_name = validated_src.file_name().ok_or("Cannot get file name")?;
    let new_path = validated_dest.join(file_name);
    debug!("Copy destination path: {:?}", new_path);

    if new_path.exists() {
        warn!(
            "Copy failed: destination file already exists: {:?}",
            new_path
        );
        return Err(format!(
            "A file named '{}' already exists in destination",
            file_name.to_string_lossy()
        ));
    }

    if validated_src.is_dir() {
        debug!("Copying directory recursively: {:?}", validated_src);
        copy_dir_recursive(&validated_src, &new_path).map_err(|e| {
            warn!("Failed to copy folder {:?}: {}", validated_src, e);
            format!("Failed to copy folder: {}", e)
        })?;
    } else {
        debug!("Copying file: {:?}", validated_src);
        fs::copy(&validated_src, &new_path).map_err(|e| {
            warn!("Failed to copy file {:?}: {}", validated_src, e);
            format!("Failed to copy: {}", e)
        })?;
    }

    info!("Successfully copied {:?} to {:?}", validated_src, new_path);

    Ok(FileOperationResult {
        success: true,
        message: format!("Copied to '{}'", destination),
        new_path: Some(new_path.to_string_lossy().to_string()),
    })
}

/// Recursively copy a directory
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
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

/// Create a new folder
pub fn create_folder(path: String, name: String) -> Result<FileOperationResult, String> {
    info!("create_folder: path={:?}, name={:?}", path, name);

    let parent_path = Path::new(&path);

    // Validate parent path is within allowed directories
    let validated_parent = config::validate_path(parent_path)?;

    if !validated_parent.is_dir() {
        warn!(
            "Create folder failed: parent is not a directory: {:?}",
            validated_parent
        );
        return Err("Parent path must be a directory".to_string());
    }

    let new_folder = validated_parent.join(&name);
    debug!("Creating folder at: {:?}", new_folder);

    // Validate new folder path is also within allowed directories
    config::validate_path(&new_folder)?;
    if new_folder.exists() {
        warn!(
            "Create folder failed: folder already exists: {:?}",
            new_folder
        );
        return Err(format!("A folder named '{}' already exists", name));
    }

    fs::create_dir(&new_folder).map_err(|e| {
        warn!("Failed to create folder {:?}: {}", new_folder, e);
        format!("Failed to create folder: {}", e)
    })?;

    info!("Successfully created folder: {:?}", new_folder);

    Ok(FileOperationResult {
        success: true,
        message: format!("Created folder '{}'", name),
        new_path: Some(new_folder.to_string_lossy().to_string()),
    })
}

/// Reveal a file in the system file manager
pub fn reveal_in_folder(path: String) -> Result<FileOperationResult, String> {
    let file_path = Path::new(&path);

    // Validate path is within allowed directories
    let validated_path = config::validate_path(file_path)?;

    if !validated_path.exists() {
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
