use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

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
        .invoke_handler(tauri::generate_handler![
            list_directory,
            get_file_metadata,
            get_home_dir,
            check_dependencies
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
