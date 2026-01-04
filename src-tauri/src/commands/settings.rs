//! Settings-related Tauri commands
//!
//! This module contains all Tauri commands for managing application settings.
//! Settings are stored in the SQLite database and can be accessed from both
//! the frontend and backend.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use tauri::Manager;

/// Result of path validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathValidationResult {
    pub valid: bool,
    pub exists: bool,
    pub created: bool,
    pub path: String,
    pub error: Option<String>,
}

/// Get the initial directory for the app based on settings
#[tauri::command]
pub async fn get_initial_directory(app: tauri::AppHandle) -> Result<String, String> {
    let db_path = app
        .path()
        .app_data_dir()
        .map_err(|e: tauri::Error| e.to_string())?
        .join("seer.db");

    // Check if database exists yet
    if !db_path.exists() {
        // Return home directory if DB doesn't exist yet
        return Ok(dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string()));
    }

    // Try to get settings from the database via SQL plugin
    // For now, we'll use the home directory as fallback
    // The frontend will handle the actual DB query after initialization
    Ok(dirs::home_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string()))
}

/// Validate a path and optionally create missing directories
#[tauri::command]
pub fn validate_path(path: String, create_if_missing: bool) -> PathValidationResult {
    let path = expand_tilde(&path);
    let path_obj = Path::new(&path);

    // Check if path is valid (not empty and has valid characters)
    if path.is_empty() {
        return PathValidationResult {
            valid: false,
            exists: false,
            created: false,
            path: path.clone(),
            error: Some("Path cannot be empty".to_string()),
        };
    }

    // Check if path exists
    if path_obj.exists() {
        if path_obj.is_dir() {
            return PathValidationResult {
                valid: true,
                exists: true,
                created: false,
                path,
                error: None,
            };
        } else {
            return PathValidationResult {
                valid: false,
                exists: true,
                created: false,
                path,
                error: Some("Path exists but is not a directory".to_string()),
            };
        }
    }

    // Path doesn't exist - try to create if requested
    if create_if_missing {
        match fs::create_dir_all(&path) {
            Ok(_) => {
                log::info!("Created directory: {}", path);
                PathValidationResult {
                    valid: true,
                    exists: true,
                    created: true,
                    path,
                    error: None,
                }
            }
            Err(e) => {
                log::error!("Failed to create directory {}: {}", path, e);
                PathValidationResult {
                    valid: false,
                    exists: false,
                    created: false,
                    path,
                    error: Some(format!("Failed to create directory: {}", e)),
                }
            }
        }
    } else {
        PathValidationResult {
            valid: false,
            exists: false,
            created: false,
            path,
            error: Some("Directory does not exist".to_string()),
        }
    }
}

/// Open a native folder picker dialog and return the selected path
#[tauri::command]
pub async fn pick_folder(
    app: tauri::AppHandle,
    title: Option<String>,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let mut dialog = app.dialog().file();

    if let Some(t) = title {
        dialog = dialog.set_title(&t);
    }

    if let Some(path) = default_path {
        let expanded = expand_tilde(&path);
        let path_obj = std::path::PathBuf::from(&expanded);
        if path_obj.exists() {
            dialog = dialog.set_directory(&path_obj);
        }
    }

    // Use blocking_pick_folder for synchronous folder selection
    let result = dialog.blocking_pick_folder();

    match result {
        Some(path) => Ok(Some(path.to_string())),
        None => Ok(None), // User cancelled
    }
}

/// Save the last visited directory
#[tauri::command]
pub fn save_last_directory(path: String) -> Result<(), String> {
    // Validate the path exists
    let expanded = expand_tilde(&path);
    if !Path::new(&expanded).exists() {
        return Err(format!("Directory does not exist: {}", path));
    }

    // The actual save to DB will be done via the SQL plugin from frontend
    // This command is mainly for validation
    log::info!("Last directory saved: {}", path);
    Ok(())
}

/// Expand ~ to home directory
fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return path.replacen('~', &home.to_string_lossy(), 1);
        }
    }
    path.to_string()
}

/// Get the default downloads directory
#[tauri::command]
pub fn get_default_downloads_dir() -> String {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "~/Downloads".to_string())
}
