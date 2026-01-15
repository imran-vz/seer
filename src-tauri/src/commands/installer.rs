//! Installer-related Tauri commands

use crate::installer::{self, Tool};
use crate::types::{InstallMethod, InstallProgress, InstallResult, InstallStrategy};
use log::{debug, error, info};
use tauri::{Emitter, Manager};

/// Get available installation strategies for a tool
#[tauri::command]
pub async fn get_install_strategies(tool: String) -> Result<Vec<InstallStrategy>, String> {
    debug!("get_install_strategies called for tool: {}", tool);

    let tool_enum = Tool::from_string(&tool)?;
    let strategies = installer::get_install_strategies(&tool_enum);

    Ok(strategies)
}

/// Install a dependency (FFmpeg or ExifTool)
#[tauri::command]
pub async fn install_dependency(
    app: tauri::AppHandle,
    tool: String,
    method: Option<String>,
) -> Result<InstallResult, String> {
    info!(
        "install_dependency called: tool={}, method={:?}",
        tool, method
    );

    let tool_enum = Tool::from_string(&tool)?;

    // Parse preferred method if provided
    let preferred_method = if let Some(m) = method {
        Some(parse_install_method(&m)?)
    } else {
        None
    };

    // Get app data directory
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    // Create progress callback that emits events to frontend
    let app_handle = app.clone();
    let tool_name = tool.clone();
    let progress_callback = move |progress: InstallProgress| {
        debug!(
            "Install progress for {}: {}% - {}",
            progress.tool, progress.percentage, progress.stage
        );

        // Emit progress event to frontend
        let _ = app_handle.emit(&format!("install-progress-{}", tool_name), &progress);
    };

    // Perform installation
    let result = installer::install_tool(
        &tool_enum,
        preferred_method,
        progress_callback,
        app_data_dir,
    )
    .await;

    // Emit completion event
    let _ = app.emit(&format!("install-complete-{}", tool), &result);

    Ok(result)
}

/// Cancel an ongoing installation
#[tauri::command]
pub fn cancel_installation(tool: String) -> Result<bool, String> {
    debug!("cancel_installation called for tool: {}", tool);

    let cancelled = installer::cancel_installation(&tool);

    if cancelled {
        info!("Installation of {} cancelled", tool);
    } else {
        error!("Failed to cancel installation of {}", tool);
    }

    Ok(cancelled)
}

/// Parse install method string to enum
fn parse_install_method(method: &str) -> Result<InstallMethod, String> {
    match method.to_lowercase().as_str() {
        "homebrew" => Ok(InstallMethod::Homebrew),
        "winget" => Ok(InstallMethod::Winget),
        "chocolatey" => Ok(InstallMethod::Chocolatey),
        "scoop" => Ok(InstallMethod::Scoop),
        "apt" => Ok(InstallMethod::Apt),
        "dnf" => Ok(InstallMethod::Dnf),
        "pacman" => Ok(InstallMethod::Pacman),
        "snap" => Ok(InstallMethod::Snap),
        "direct_download" | "directdownload" => Ok(InstallMethod::DirectDownload),
        _ => Err(format!("Unknown install method: {}", method)),
    }
}
