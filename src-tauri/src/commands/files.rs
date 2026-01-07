//! File-related Tauri commands
//!
//! This module contains all Tauri commands for file system operations.

use crate::files;
use crate::files::filters::{FilterCriteria, FilterResult};
use crate::types::{
    BulkRenameResult, DependenciesResult, FileEntry, FileMetadata, FileOperationResult,
    RenamePattern, RenamePreview,
};

#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<FileEntry>, String> {
    files::list_directory(path)
}

#[tauri::command]
pub fn get_file_metadata(path: String) -> Result<FileMetadata, String> {
    files::get_file_metadata(path)
}

#[tauri::command]
pub fn get_home_dir() -> String {
    files::get_home_dir()
}

#[tauri::command]
pub fn check_dependencies() -> DependenciesResult {
    files::check_dependencies()
}

#[tauri::command]
pub fn rename_file(path: String, new_name: String) -> Result<FileOperationResult, String> {
    files::rename_file(path, new_name)
}

#[tauri::command]
pub fn delete_file(path: String, permanent: bool) -> Result<FileOperationResult, String> {
    files::delete_file(path, permanent)
}

#[tauri::command]
pub fn move_file(path: String, destination: String) -> Result<FileOperationResult, String> {
    files::move_file(path, destination)
}

#[tauri::command]
pub fn copy_file(path: String, destination: String) -> Result<FileOperationResult, String> {
    files::copy_file(path, destination)
}

#[tauri::command]
pub fn create_folder(path: String, name: String) -> Result<FileOperationResult, String> {
    files::create_folder(path, name)
}

#[tauri::command]
pub fn reveal_in_folder(path: String) -> Result<FileOperationResult, String> {
    files::reveal_in_folder(path)
}

#[tauri::command]
pub async fn preview_bulk_rename(
    paths: Vec<String>,
    pattern: RenamePattern,
    auto_rename_conflicts: bool,
) -> Result<Vec<RenamePreview>, String> {
    // Run in blocking task to avoid blocking UI thread
    tokio::task::spawn_blocking(move || {
        files::preview_renames(paths, pattern, auto_rename_conflicts)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn execute_bulk_rename(
    paths: Vec<String>,
    pattern: RenamePattern,
    auto_rename_conflicts: bool,
) -> Result<BulkRenameResult, String> {
    // Run in blocking task to avoid blocking UI thread
    tokio::task::spawn_blocking(move || {
        // First get previews
        let previews = files::preview_renames(paths.clone(), pattern, auto_rename_conflicts)?;

        // Check for errors or conflicts
        let has_errors = previews.iter().any(|p| p.error.is_some());
        let has_conflicts = previews.iter().any(|p| p.conflict);

        if has_errors {
            let errors: Vec<String> = previews.iter().filter_map(|p| p.error.clone()).collect();
            return Err(format!("Validation errors: {}", errors.join(", ")));
        }

        if has_conflicts {
            return Err(
                "Naming conflicts detected. Enable auto-rename or fix manually.".to_string(),
            );
        }

        // Execute renames
        let mut success = 0;
        let mut failed = 0;
        let mut errors = Vec::new();

        for preview in previews {
            // Skip if name unchanged
            if preview.original_path == preview.new_path {
                success += 1;
                continue;
            }

            match std::fs::rename(&preview.original_path, &preview.new_path) {
                Ok(_) => success += 1,
                Err(e) => {
                    failed += 1;
                    errors.push(format!("{}: {}", preview.original_name, e));
                }
            }
        }

        Ok(BulkRenameResult {
            success,
            failed,
            errors,
        })
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn create_folders_from_selection(
    paths: Vec<String>,
    mode: crate::types::FolderCreationMode,
    parent_dir: String,
) -> Result<crate::types::FolderCreationResult, String> {
    // Run in blocking task to avoid blocking UI thread
    tokio::task::spawn_blocking(move || {
        files::create_folders_from_selection(paths, mode, parent_dir)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn apply_filters(
    current_dir: String,
    filters: FilterCriteria,
) -> Result<FilterResult, String> {
    // Run in blocking task to avoid blocking UI thread (media filters may call ffprobe)
    tokio::task::spawn_blocking(move || files::apply_filters(current_dir, filters))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn get_available_extensions(current_dir: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || files::get_available_extensions(current_dir))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}
