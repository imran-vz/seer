//! File system operations module
//!
//! This module handles file system operations including:
//! - Directory listing
//! - File metadata retrieval
//! - File operations (rename, delete, move, copy)
//! - Folder creation
//! - Revealing files in system file manager
//! - Bulk rename operations with various patterns
//! - Folder creation from selections (per-file, grouped, single)
//! - Smart filtering by size, date, extension, and media properties

pub mod filters;
pub mod folder_operations;
mod operations;
pub mod rename_patterns;

pub use operations::{
    check_command, check_dependencies, copy_dir_recursive, copy_file, create_folder, delete_file,
    format_time, get_file_metadata, get_home_dir, is_media_file, list_directory, move_file,
    rename_file, reveal_in_folder,
};

pub use filters::{apply_filters, get_available_extensions, FilterCriteria, FilterResult};
pub use folder_operations::create_folders_from_selection;
pub use rename_patterns::preview_renames;
