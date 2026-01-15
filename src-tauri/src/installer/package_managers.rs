//! Package manager installation implementations

use crate::installer::Tool;
use crate::types::{InstallMethod, InstallProgress, InstallResult};
use log::{error, info};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, timeout};

const INSTALL_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Spawn a background task to send simulated progress updates
async fn simulate_progress<F>(
    tool: &Tool,
    method: &InstallMethod,
    progress_callback: F,
    cancel_flag: Arc<AtomicBool>,
) where
    F: Fn(InstallProgress) + Send + 'static,
{
    let tool_name = tool.name().to_string();
    let method_clone = method.clone();

    tokio::spawn(async move {
        let mut progress_interval = interval(Duration::from_secs(3));
        let mut current_progress = 35.0;

        loop {
            progress_interval.tick().await;

            if cancel_flag.load(Ordering::SeqCst) {
                break;
            }

            // Increment progress slowly (max 90%)
            current_progress += 5.0;
            if current_progress > 90.0 {
                current_progress = 90.0;
            }

            progress_callback(InstallProgress {
                tool: tool_name.clone(),
                method: method_clone.clone(),
                current: current_progress as usize,
                total: 100,
                percentage: current_progress,
                stage: format!("Installing... {}%", current_progress as usize),
                logs: vec![],
            });
        }
    });
}

/// Install tool via package manager
pub async fn install_via_package_manager<F>(
    tool: &Tool,
    method: &InstallMethod,
    progress_callback: F,
) -> InstallResult
where
    F: Fn(InstallProgress) + Send + Sync + 'static,
{
    info!("Installing {} via {:?}", tool.display_name(), method);

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: method.clone(),
        current: 10,
        total: 100,
        percentage: 10.0,
        stage: format!("Preparing to install via {:?}...", method),
        logs: vec![],
    });

    let result = match method {
        InstallMethod::Homebrew => install_homebrew(tool, progress_callback).await,
        InstallMethod::Winget => install_winget(tool, progress_callback).await,
        InstallMethod::Chocolatey => install_chocolatey(tool, progress_callback).await,
        InstallMethod::Scoop => install_scoop(tool, progress_callback).await,
        InstallMethod::Apt => install_apt(tool, progress_callback).await,
        InstallMethod::Dnf => install_dnf(tool, progress_callback).await,
        InstallMethod::Pacman => install_pacman(tool, progress_callback).await,
        InstallMethod::Snap => install_snap(tool, progress_callback).await,
        _ => InstallResult {
            success: false,
            tool: tool.name().to_string(),
            method: method.clone(),
            message: format!("Package manager {:?} not implemented", method),
            installed_path: None,
            version: None,
        },
    };

    result
}

/// Install via Homebrew (macOS)
async fn install_homebrew(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    let package_name = match tool {
        Tool::FFmpeg => "ffmpeg",
        Tool::ExifTool => "exiftool",
    };

    // Wrap callback in Arc for sharing
    let callback = Arc::new(progress_callback);

    callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Homebrew,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: format!("Running: brew install {}...", package_name),
        logs: vec![format!("$ brew install {}", package_name)],
    });

    // Start simulated progress updates
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_clone = cancel_flag.clone();
    let callback_clone = callback.clone();
    simulate_progress(
        tool,
        &InstallMethod::Homebrew,
        move |p| callback_clone(p),
        cancel_clone,
    )
    .await;

    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("brew")
                .args(["install", package_name])
                .output()
        }),
    )
    .await;

    // Stop simulated progress
    cancel_flag.store(true, Ordering::SeqCst);

    match result {
        Ok(Ok(Ok(output))) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                info!("Homebrew install successful: {}", stdout);

                callback(InstallProgress {
                    tool: tool.name().to_string(),
                    method: InstallMethod::Homebrew,
                    current: 100,
                    total: 100,
                    percentage: 100.0,
                    stage: "Installation complete!".to_string(),
                    logs: vec![stdout.to_string()],
                });

                InstallResult {
                    success: true,
                    tool: tool.name().to_string(),
                    method: InstallMethod::Homebrew,
                    message: format!(
                        "{} installed successfully via Homebrew",
                        tool.display_name()
                    ),
                    installed_path: Some(format!("/opt/homebrew/bin/{}", package_name)),
                    version: None,
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Homebrew install failed: {}", stderr);

                InstallResult {
                    success: false,
                    tool: tool.name().to_string(),
                    method: InstallMethod::Homebrew,
                    message: format!("Installation failed: {}", stderr),
                    installed_path: None,
                    version: None,
                }
            }
        }
        _ => InstallResult {
            success: false,
            tool: tool.name().to_string(),
            method: InstallMethod::Homebrew,
            message: "Installation timed out or failed to execute".to_string(),
            installed_path: None,
            version: None,
        },
    }
}

/// Install via winget (Windows)
async fn install_winget(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    let package_id = match tool {
        Tool::FFmpeg => "Gyan.FFmpeg",
        Tool::ExifTool => "Oliver-Scherer.ExifTool",
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Winget,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: format!("Running: winget install {}...", package_id),
        logs: vec![format!("$ winget install {}", package_id)],
    });

    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("winget")
                .args([
                    "install",
                    "--id",
                    package_id,
                    "--silent",
                    "--accept-source-agreements",
                ])
                .output()
        }),
    )
    .await;

    handle_command_result(result, tool, &InstallMethod::Winget, progress_callback)
}

/// Install via Chocolatey (Windows, requires admin)
async fn install_chocolatey(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    let package_name = match tool {
        Tool::FFmpeg => "ffmpeg",
        Tool::ExifTool => "exiftool",
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Chocolatey,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: format!("Running: choco install {}...", package_name),
        logs: vec![format!("$ choco install {} -y", package_name)],
    });

    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("choco")
                .args(["install", package_name, "-y"])
                .output()
        }),
    )
    .await;

    handle_command_result(result, tool, &InstallMethod::Chocolatey, progress_callback)
}

/// Install via Scoop (Windows)
async fn install_scoop(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    let package_name = match tool {
        Tool::FFmpeg => "ffmpeg",
        Tool::ExifTool => "exiftool",
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Scoop,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: format!("Running: scoop install {}...", package_name),
        logs: vec![format!("$ scoop install {}", package_name)],
    });

    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("scoop")
                .args(["install", package_name])
                .output()
        }),
    )
    .await;

    handle_command_result(result, tool, &InstallMethod::Scoop, progress_callback)
}

/// Install via APT (Debian/Ubuntu Linux)
async fn install_apt(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    let package_name = match tool {
        Tool::FFmpeg => "ffmpeg",
        Tool::ExifTool => "libimage-exiftool-perl",
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Apt,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: format!("Running: sudo apt install {}...", package_name),
        logs: vec![format!(
            "$ sudo apt update && sudo apt install -y {}",
            package_name
        )],
    });

    // Note: This requires sudo and may prompt for password
    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("sudo")
                .args(["apt", "install", "-y", package_name])
                .output()
        }),
    )
    .await;

    handle_command_result(result, tool, &InstallMethod::Apt, progress_callback)
}

/// Install via DNF (Fedora/RHEL Linux)
async fn install_dnf(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    let package_name = match tool {
        Tool::FFmpeg => "ffmpeg",
        Tool::ExifTool => "perl-Image-ExifTool",
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Dnf,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: format!("Running: sudo dnf install {}...", package_name),
        logs: vec![format!("$ sudo dnf install -y {}", package_name)],
    });

    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("sudo")
                .args(["dnf", "install", "-y", package_name])
                .output()
        }),
    )
    .await;

    handle_command_result(result, tool, &InstallMethod::Dnf, progress_callback)
}

/// Install via Pacman (Arch Linux)
async fn install_pacman(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    let package_name = match tool {
        Tool::FFmpeg => "ffmpeg",
        Tool::ExifTool => "perl-image-exiftool",
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Pacman,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: format!("Running: sudo pacman -S {}...", package_name),
        logs: vec![format!("$ sudo pacman -S --noconfirm {}", package_name)],
    });

    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("sudo")
                .args(["pacman", "-S", "--noconfirm", package_name])
                .output()
        }),
    )
    .await;

    handle_command_result(result, tool, &InstallMethod::Pacman, progress_callback)
}

/// Install via Snap (Linux)
async fn install_snap(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    if !matches!(tool, Tool::FFmpeg) {
        return InstallResult {
            success: false,
            tool: tool.name().to_string(),
            method: InstallMethod::Snap,
            message: "ExifTool not available via Snap".to_string(),
            installed_path: None,
            version: None,
        };
    }

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::Snap,
        current: 30,
        total: 100,
        percentage: 30.0,
        stage: "Running: sudo snap install ffmpeg...".to_string(),
        logs: vec!["$ sudo snap install ffmpeg".to_string()],
    });

    let result = timeout(
        INSTALL_TIMEOUT,
        tokio::task::spawn_blocking(move || {
            Command::new("sudo")
                .args(["snap", "install", "ffmpeg"])
                .output()
        }),
    )
    .await;

    handle_command_result(result, tool, &InstallMethod::Snap, progress_callback)
}

/// Handle command execution result
fn handle_command_result(
    result: Result<
        Result<Result<std::process::Output, std::io::Error>, tokio::task::JoinError>,
        tokio::time::error::Elapsed,
    >,
    tool: &Tool,
    method: &InstallMethod,
    progress_callback: impl Fn(InstallProgress) + Send + Sync + 'static,
) -> InstallResult {
    match result {
        Ok(Ok(Ok(output))) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                info!("Installation successful via {:?}: {}", method, stdout);

                progress_callback(InstallProgress {
                    tool: tool.name().to_string(),
                    method: method.clone(),
                    current: 100,
                    total: 100,
                    percentage: 100.0,
                    stage: "Installation complete!".to_string(),
                    logs: vec![stdout.to_string()],
                });

                InstallResult {
                    success: true,
                    tool: tool.name().to_string(),
                    method: method.clone(),
                    message: format!("{} installed successfully", tool.display_name()),
                    installed_path: None,
                    version: None,
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                error!("Installation failed via {:?}: {}", method, stderr);

                InstallResult {
                    success: false,
                    tool: tool.name().to_string(),
                    method: method.clone(),
                    message: format!("Installation failed: {}", stderr),
                    installed_path: None,
                    version: None,
                }
            }
        }
        _ => {
            error!(
                "Installation timed out or failed to execute via {:?}",
                method
            );

            InstallResult {
                success: false,
                tool: tool.name().to_string(),
                method: method.clone(),
                message: "Installation timed out or failed to execute".to_string(),
                installed_path: None,
                version: None,
            }
        }
    }
}
