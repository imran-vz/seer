use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v",
    "mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "opus",
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "heic",
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

    files.sort_by(|a, b| {
        match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        }
    });

    Ok(files)
}

#[tauri::command]
fn get_file_metadata(path: String) -> Result<FileMetadata, String> {
    let file_path = Path::new(&path);
    let metadata = fs::metadata(&file_path).map_err(|e| e.to_string())?;
    
    let is_media = is_media_file(file_path);
    
    let ffprobe_data = if is_media {
        Command::new("ffprobe")
            .args(["-v", "quiet", "-print_format", "json", "-show_format", "-show_streams", &path])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
    } else {
        None
    };

    Ok(FileMetadata {
        path: path.clone(),
        name: file_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
        size: metadata.len(),
        modified: format_time(metadata.modified()),
        created: format_time(metadata.created()),
        is_media,
        extension: file_path.extension().map(|e| e.to_string_lossy().to_string()),
        ffprobe_data,
    })
}

#[tauri::command]
fn get_home_dir() -> String {
    dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![list_directory, get_file_metadata, get_home_dir])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
