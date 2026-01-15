use crate::metadata;
use crate::types::{
    MetadataOperation, MetadataSnapshot, MetadataToolAvailability, MetadataUpdateResult,
};

#[tauri::command]
pub fn list_metadata(path: String) -> Result<MetadataSnapshot, String> {
    metadata::list_metadata(path)
}

#[tauri::command]
pub async fn update_metadata(
    path: String,
    operations: Vec<MetadataOperation>,
) -> Result<MetadataUpdateResult, String> {
    tauri::async_runtime::spawn_blocking(move || metadata::update_metadata(path, operations))
        .await
        .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub fn metadata_tools() -> MetadataToolAvailability {
    metadata::tool_status()
}
