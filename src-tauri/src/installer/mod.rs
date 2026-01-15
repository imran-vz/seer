//! Dependency installation module
//!
//! Handles automatic installation of FFmpeg and ExifTool across platforms.
//! Uses native package managers when available, falls back to direct downloads.

pub mod downloads;
pub mod package_managers;
pub mod platforms;

use crate::types::{InstallMethod, InstallProgress, InstallResult, InstallStrategy};
use log::{debug, error, info, warn};
use std::path::PathBuf;

/// Supported tools for installation
#[derive(Debug, Clone, PartialEq)]
pub enum Tool {
    FFmpeg,
    ExifTool,
}

impl Tool {
    pub fn from_string(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "ffmpeg" => Ok(Tool::FFmpeg),
            "exiftool" => Ok(Tool::ExifTool),
            _ => Err(format!("Unknown tool: {}", s)),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Tool::FFmpeg => "ffmpeg",
            Tool::ExifTool => "exiftool",
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            Tool::FFmpeg => "FFmpeg",
            Tool::ExifTool => "ExifTool",
        }
    }
}

/// Get available installation strategies for a tool on current platform
pub fn get_install_strategies(tool: &Tool) -> Vec<InstallStrategy> {
    platforms::detect_available_strategies(tool)
}

/// Install a tool using the best available method
pub async fn install_tool(
    tool: &Tool,
    preferred_method: Option<InstallMethod>,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
    app_data_dir: PathBuf,
) -> InstallResult {
    info!("Starting installation of {}", tool.display_name());

    let strategies = get_install_strategies(tool);

    if strategies.is_empty() {
        error!("No installation strategies available for {}", tool.name());
        return InstallResult {
            success: false,
            tool: tool.name().to_string(),
            method: InstallMethod::DirectDownload,
            message: "No installation methods available for your platform".to_string(),
            installed_path: None,
            version: None,
        };
    }

    // Find the preferred method or use the first available
    let strategy = if let Some(method) = preferred_method {
        strategies
            .iter()
            .find(|s| s.method == method && s.available)
            .or_else(|| strategies.first())
    } else {
        strategies.first()
    };

    let strategy = match strategy {
        Some(s) => s,
        None => {
            return InstallResult {
                success: false,
                tool: tool.name().to_string(),
                method: InstallMethod::DirectDownload,
                message: "No available installation method found".to_string(),
                installed_path: None,
                version: None,
            };
        }
    };

    info!(
        "Using installation method: {:?} for {}",
        strategy.method,
        tool.name()
    );

    // Wrap callback in Arc for sharing across attempts
    use std::sync::Arc;
    let callback = Arc::new(progress_callback);

    // Attempt installation with the selected strategy
    let callback_clone = callback.clone();
    let result = match strategy.method {
        InstallMethod::Homebrew
        | InstallMethod::Winget
        | InstallMethod::Chocolatey
        | InstallMethod::Scoop
        | InstallMethod::Apt
        | InstallMethod::Dnf
        | InstallMethod::Pacman
        | InstallMethod::Snap => {
            package_managers::install_via_package_manager(tool, &strategy.method, move |p| {
                callback_clone(p)
            })
            .await
        }
        InstallMethod::DirectDownload => {
            let callback_clone2 = callback.clone();
            downloads::install_via_direct_download(
                tool,
                move |p| callback_clone2(p),
                app_data_dir.clone(),
            )
            .await
        }
    };

    // If installation failed and we have fallbacks, try them
    if !result.success && strategies.len() > 1 {
        warn!(
            "Installation failed with {:?}, trying fallback methods",
            strategy.method
        );

        for fallback_strategy in strategies.iter().skip(1) {
            if !fallback_strategy.available {
                continue;
            }

            info!(
                "Attempting fallback installation with {:?}",
                fallback_strategy.method
            );

            let callback_clone = callback.clone();
            let fallback_result = match fallback_strategy.method {
                InstallMethod::DirectDownload => {
                    downloads::install_via_direct_download(
                        tool,
                        move |p| callback_clone(p),
                        app_data_dir.clone(),
                    )
                    .await
                }
                _ => {
                    package_managers::install_via_package_manager(
                        tool,
                        &fallback_strategy.method,
                        move |p| callback_clone(p),
                    )
                    .await
                }
            };

            if fallback_result.success {
                return fallback_result;
            }
        }
    }

    result
}

/// Cancel an ongoing installation
pub fn cancel_installation(tool: &str) -> bool {
    debug!("Attempting to cancel installation of {}", tool);
    // This will be integrated with the job queue system
    // For now, return false as we don't track cancellations yet
    false
}
