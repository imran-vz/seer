//! File system operations
//!
//! This module handles:
//! - Directory listing
//! - File metadata retrieval
//! - File operations (rename, delete, move, copy)
//! - Dependency checking

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::media::find_command;
use crate::types::{
    DependenciesResult, DependencyStatus, FileEntry, FileMetadata, FileOperationResult,
    MEDIA_EXTENSIONS,
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

/// Get file metadata including ffprobe data for media files
pub fn get_file_metadata(path: String) -> Result<FileMetadata, String> {
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

/// Delete a file (to trash or permanently)
pub fn delete_file(path: String, permanent: bool) -> Result<FileOperationResult, String> {
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

/// Move a file to a new location
pub fn move_file(path: String, destination: String) -> Result<FileOperationResult, String> {
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

/// Copy a file to a new location
pub fn copy_file(path: String, destination: String) -> Result<FileOperationResult, String> {
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

/// Reveal a file in the system file manager
pub fn reveal_in_folder(path: String) -> Result<FileOperationResult, String> {
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
