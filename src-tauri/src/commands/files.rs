//! File-related Tauri commands
//!
//! This module contains all Tauri commands for file system operations.

use crate::files;
use crate::types::{DependenciesResult, FileEntry, FileMetadata, FileOperationResult};

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
