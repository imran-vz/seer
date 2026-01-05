//! Application configuration
//!
//! Manages user settings including:
//! - Allowed directories for file operations (whitelist)
//! - Security settings

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Whitelist of directories allowed for file operations
    /// Empty list = unrestricted (first-time setup)
    pub allowed_directories: Vec<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        // Start with home directory as default allowed directory
        let mut allowed = Vec::new();
        if let Some(home) = dirs::home_dir() {
            allowed.push(home);
        }
        Self {
            allowed_directories: allowed,
        }
    }
}

/// Global configuration instance
static CONFIG: Lazy<RwLock<AppConfig>> = Lazy::new(|| RwLock::new(AppConfig::default()));

/// Validate that a path is within allowed directories
/// Returns canonicalized path if valid, error otherwise
pub fn validate_path(path: &Path) -> Result<PathBuf, String> {
    let config = CONFIG.read().map_err(|e| format!("Config lock error: {}", e))?;

    // Canonicalize the path (resolves symlinks, .. , etc.)
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;

    // If no allowed directories configured, prompt user to configure
    if config.allowed_directories.is_empty() {
        return Err("No allowed directories configured. Please configure allowed directories in settings.".to_string());
    }

    // Check if path is within any allowed directory
    for allowed_dir in &config.allowed_directories {
        // Canonicalize allowed directory
        if let Ok(allowed_canonical) = allowed_dir.canonicalize() {
            if canonical.starts_with(&allowed_canonical) {
                return Ok(canonical);
            }
        }
    }

    Err(format!(
        "Access denied: path '{}' is not within allowed directories",
        path.display()
    ))
}

/// Add a directory to the whitelist
pub fn add_allowed_directory(path: PathBuf) -> Result<(), String> {
    let mut config = CONFIG.write().map_err(|e| format!("Config lock error: {}", e))?;

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Invalid directory: {}", e))?;

    if !canonical.is_dir() {
        return Err("Path must be a directory".to_string());
    }

    if !config.allowed_directories.contains(&canonical) {
        config.allowed_directories.push(canonical);
    }

    Ok(())
}

/// Remove a directory from the whitelist
pub fn remove_allowed_directory(path: &Path) -> Result<(), String> {
    let mut config = CONFIG.write().map_err(|e| format!("Config lock error: {}", e))?;

    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Invalid directory: {}", e))?;

    config.allowed_directories.retain(|d| d != &canonical);

    Ok(())
}

/// Get list of allowed directories
pub fn get_allowed_directories() -> Result<Vec<PathBuf>, String> {
    let config = CONFIG.read().map_err(|e| format!("Config lock error: {}", e))?;
    Ok(config.allowed_directories.clone())
}

/// Set allowed directories (replaces existing list)
pub fn set_allowed_directories(directories: Vec<PathBuf>) -> Result<(), String> {
    let mut config = CONFIG.write().map_err(|e| format!("Config lock error: {}", e))?;

    // Validate all directories exist and are accessible
    let mut validated = Vec::new();
    for dir in directories {
        let canonical = dir
            .canonicalize()
            .map_err(|e| format!("Invalid directory '{}': {}", dir.display(), e))?;

        if !canonical.is_dir() {
            return Err(format!("Path '{}' is not a directory", dir.display()));
        }

        validated.push(canonical);
    }

    config.allowed_directories = validated;
    Ok(())
}
