//! Platform detection and installation strategy selection

use crate::installer::Tool;
use crate::types::{InstallMethod, InstallStrategy};
use log::debug;
use std::env;
use std::path::Path;
use std::process::Command;

/// Detect available installation strategies for a tool on the current platform
pub fn detect_available_strategies(tool: &Tool) -> Vec<InstallStrategy> {
    let os = env::consts::OS;
    debug!("Detecting strategies for {} on {}", tool.name(), os);

    match os {
        "macos" => detect_macos_strategies(tool),
        "windows" => detect_windows_strategies(tool),
        "linux" => detect_linux_strategies(tool),
        _ => vec![],
    }
}

/// Detect strategies for macOS
fn detect_macos_strategies(tool: &Tool) -> Vec<InstallStrategy> {
    let mut strategies = Vec::new();

    // Check for Homebrew
    if is_homebrew_available() {
        strategies.push(InstallStrategy {
            method: InstallMethod::Homebrew,
            requires_admin: false,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "/opt/homebrew/bin or /usr/local/bin".to_string(),
            estimated_size_mb: match tool {
                Tool::FFmpeg => 85,
                Tool::ExifTool => 5,
            },
        });
    }

    // Direct download is always available as fallback
    strategies.push(InstallStrategy {
        method: InstallMethod::DirectDownload,
        requires_admin: false,
        available: true,
        tool_name: tool.display_name().to_string(),
        install_location: "~/Library/Application Support/Seer/bin".to_string(),
        estimated_size_mb: match tool {
            Tool::FFmpeg => 80,
            Tool::ExifTool => 5,
        },
    });

    strategies
}

/// Detect strategies for Windows
fn detect_windows_strategies(tool: &Tool) -> Vec<InstallStrategy> {
    let mut strategies = Vec::new();

    // Check for winget (Windows 10 1809+)
    if is_winget_available() {
        strategies.push(InstallStrategy {
            method: InstallMethod::Winget,
            requires_admin: false,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "System-managed location".to_string(),
            estimated_size_mb: match tool {
                Tool::FFmpeg => 80,
                Tool::ExifTool => 15,
            },
        });
    }

    // Check for Scoop (doesn't require admin)
    if is_scoop_available() {
        strategies.push(InstallStrategy {
            method: InstallMethod::Scoop,
            requires_admin: false,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "%USERPROFILE%\\scoop\\apps".to_string(),
            estimated_size_mb: match tool {
                Tool::FFmpeg => 80,
                Tool::ExifTool => 15,
            },
        });
    }

    // Check for Chocolatey (requires admin)
    if is_chocolatey_available() {
        strategies.push(InstallStrategy {
            method: InstallMethod::Chocolatey,
            requires_admin: true,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "%ProgramData%\\chocolatey\\bin".to_string(),
            estimated_size_mb: match tool {
                Tool::FFmpeg => 80,
                Tool::ExifTool => 15,
            },
        });
    }

    // Direct download is always available as fallback
    strategies.push(InstallStrategy {
        method: InstallMethod::DirectDownload,
        requires_admin: false,
        available: true,
        tool_name: tool.display_name().to_string(),
        install_location: "%APPDATA%\\Seer\\bin".to_string(),
        estimated_size_mb: match tool {
            Tool::FFmpeg => 80,
            Tool::ExifTool => 15,
        },
    });

    strategies
}

/// Detect strategies for Linux
fn detect_linux_strategies(tool: &Tool) -> Vec<InstallStrategy> {
    let mut strategies = Vec::new();

    // Check for APT (Debian/Ubuntu)
    if is_apt_available() {
        strategies.push(InstallStrategy {
            method: InstallMethod::Apt,
            requires_admin: true,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "/usr/bin".to_string(),
            estimated_size_mb: match tool {
                Tool::FFmpeg => 50,
                Tool::ExifTool => 3,
            },
        });
    }

    // Check for DNF (Fedora/RHEL)
    if is_dnf_available() {
        strategies.push(InstallStrategy {
            method: InstallMethod::Dnf,
            requires_admin: true,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "/usr/bin".to_string(),
            estimated_size_mb: match tool {
                Tool::FFmpeg => 50,
                Tool::ExifTool => 3,
            },
        });
    }

    // Check for Pacman (Arch)
    if is_pacman_available() {
        strategies.push(InstallStrategy {
            method: InstallMethod::Pacman,
            requires_admin: true,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "/usr/bin".to_string(),
            estimated_size_mb: match tool {
                Tool::FFmpeg => 50,
                Tool::ExifTool => 3,
            },
        });
    }

    // Check for Snap
    if is_snap_available() && matches!(tool, Tool::FFmpeg) {
        // ExifTool not commonly available in Snap
        strategies.push(InstallStrategy {
            method: InstallMethod::Snap,
            requires_admin: true,
            available: true,
            tool_name: tool.display_name().to_string(),
            install_location: "/snap/bin".to_string(),
            estimated_size_mb: 60,
        });
    }

    // Direct download is always available as fallback
    strategies.push(InstallStrategy {
        method: InstallMethod::DirectDownload,
        requires_admin: false,
        available: true,
        tool_name: tool.display_name().to_string(),
        install_location: "~/.local/share/Seer/bin".to_string(),
        estimated_size_mb: match tool {
            Tool::FFmpeg => 80,
            Tool::ExifTool => 5,
        },
    });

    strategies
}

/// Check if Homebrew is available (macOS)
fn is_homebrew_available() -> bool {
    // Check common Homebrew locations
    if Path::new("/opt/homebrew/bin/brew").exists() || Path::new("/usr/local/bin/brew").exists() {
        return true;
    }

    // Try to run 'which brew'
    Command::new("which")
        .arg("brew")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if winget is available (Windows)
fn is_winget_available() -> bool {
    Command::new("where")
        .arg("winget")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Chocolatey is available (Windows)
fn is_chocolatey_available() -> bool {
    Command::new("where")
        .arg("choco")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Scoop is available (Windows)
fn is_scoop_available() -> bool {
    Command::new("where")
        .arg("scoop")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if APT is available (Debian/Ubuntu Linux)
fn is_apt_available() -> bool {
    Path::new("/usr/bin/apt") .exists()
        || Path::new("/usr/bin/apt-get").exists()
        || Path::new("/etc/debian_version").exists()
}

/// Check if DNF is available (Fedora/RHEL Linux)
fn is_dnf_available() -> bool {
    Path::new("/usr/bin/dnf").exists()
        || Path::new("/etc/fedora-release").exists()
        || Path::new("/etc/redhat-release").exists()
}

/// Check if Pacman is available (Arch Linux)
fn is_pacman_available() -> bool {
    Path::new("/usr/bin/pacman").exists() || Path::new("/etc/arch-release").exists()
}

/// Check if Snap is available (Linux)
fn is_snap_available() -> bool {
    Command::new("which")
        .arg("snap")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
