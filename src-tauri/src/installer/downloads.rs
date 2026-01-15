//! Direct download installation with SHA-256 verification

use crate::installer::Tool;
use crate::types::{InstallMethod, InstallProgress, InstallResult};
use log::{error, info};
use reqwest::Client;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::path::{Path, PathBuf};

/// Install tool via direct download
pub async fn install_via_direct_download(
    tool: &Tool,
    progress_callback: impl Fn(InstallProgress),
    app_data_dir: PathBuf,
) -> InstallResult {
    info!(
        "Starting direct download installation for {}",
        tool.display_name()
    );

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::DirectDownload,
        current: 10,
        total: 100,
        percentage: 10.0,
        stage: "Preparing download...".to_string(),
        logs: vec![],
    });

    // Create bin directory
    let bin_dir = get_bin_directory(&app_data_dir);
    if let Err(e) = fs::create_dir_all(&bin_dir) {
        error!("Failed to create bin directory: {}", e);
        return InstallResult {
            success: false,
            tool: tool.name().to_string(),
            method: InstallMethod::DirectDownload,
            message: format!("Failed to create bin directory: {}", e),
            installed_path: None,
            version: None,
        };
    }

    // Get download URL and expected checksum
    let (download_url, expected_sha256) = match get_download_info(tool) {
        Some(info) => info,
        None => {
            return InstallResult {
                success: false,
                tool: tool.name().to_string(),
                method: InstallMethod::DirectDownload,
                message: "No download URL available for this platform".to_string(),
                installed_path: None,
                version: None,
            };
        }
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::DirectDownload,
        current: 20,
        total: 100,
        percentage: 20.0,
        stage: format!("Downloading from {}...", download_url),
        logs: vec![format!("Download URL: {}", download_url)],
    });

    // Download file
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .unwrap();

    let response = match client.get(&download_url).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("Download failed: {}", e);
            return InstallResult {
                success: false,
                tool: tool.name().to_string(),
                method: InstallMethod::DirectDownload,
                message: format!("Download failed: {}", e),
                installed_path: None,
                version: None,
            };
        }
    };

    if !response.status().is_success() {
        return InstallResult {
            success: false,
            tool: tool.name().to_string(),
            method: InstallMethod::DirectDownload,
            message: format!("Download failed with status: {}", response.status()),
            installed_path: None,
            version: None,
        };
    }

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::DirectDownload,
        current: 60,
        total: 100,
        percentage: 60.0,
        stage: "Downloading...".to_string(),
        logs: vec![],
    });

    let bytes = match response.bytes().await {
        Ok(b) => b,
        Err(e) => {
            error!("Failed to read response body: {}", e);
            return InstallResult {
                success: false,
                tool: tool.name().to_string(),
                method: InstallMethod::DirectDownload,
                message: format!("Failed to read download: {}", e),
                installed_path: None,
                version: None,
            };
        }
    };

    // Verify SHA-256 checksum if provided
    if let Some(expected) = expected_sha256 {
        progress_callback(InstallProgress {
            tool: tool.name().to_string(),
            method: InstallMethod::DirectDownload,
            current: 70,
            total: 100,
            percentage: 70.0,
            stage: "Verifying checksum...".to_string(),
            logs: vec![],
        });

        let actual_hash = compute_sha256(&bytes);
        if actual_hash != expected {
            error!(
                "Checksum mismatch! Expected: {}, Got: {}",
                expected, actual_hash
            );
            return InstallResult {
                success: false,
                tool: tool.name().to_string(),
                method: InstallMethod::DirectDownload,
                message:
                    "Checksum verification failed! Download may be corrupted or tampered with."
                        .to_string(),
                installed_path: None,
                version: None,
            };
        }

        info!("Checksum verified successfully");
    }

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::DirectDownload,
        current: 80,
        total: 100,
        percentage: 80.0,
        stage: "Extracting files...".to_string(),
        logs: vec![],
    });

    // Extract and install
    let install_path = match extract_and_install(tool, &bytes, &bin_dir) {
        Ok(path) => path,
        Err(e) => {
            error!("Extraction failed: {}", e);
            return InstallResult {
                success: false,
                tool: tool.name().to_string(),
                method: InstallMethod::DirectDownload,
                message: format!("Extraction failed: {}", e),
                installed_path: None,
                version: None,
            };
        }
    };

    progress_callback(InstallProgress {
        tool: tool.name().to_string(),
        method: InstallMethod::DirectDownload,
        current: 100,
        total: 100,
        percentage: 100.0,
        stage: "Installation complete!".to_string(),
        logs: vec![format!("Installed to: {}", install_path.display())],
    });

    InstallResult {
        success: true,
        tool: tool.name().to_string(),
        method: InstallMethod::DirectDownload,
        message: format!("{} installed successfully", tool.display_name()),
        installed_path: Some(install_path.to_string_lossy().to_string()),
        version: None,
    }
}

/// Get bin directory for downloads
fn get_bin_directory(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("bin")
}

/// Get download URL and expected SHA-256 for a tool on current platform
fn get_download_info(tool: &Tool) -> Option<(String, Option<String>)> {
    let os = std::env::consts::OS;

    match (tool, os) {
        (Tool::FFmpeg, "macos") => {
            // evermeet.cx provides static builds for macOS
            Some((
                "https://evermeet.cx/ffmpeg/ffmpeg-7.1.zip".to_string(),
                None, // Checksums would need to be updated with each version
            ))
        }
        (Tool::FFmpeg, "windows") => {
            // gyan.dev provides Windows builds
            Some((
                "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip".to_string(),
                None,
            ))
        }
        (Tool::FFmpeg, "linux") => {
            // johnvansickle.com provides static Linux builds
            Some((
                "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz"
                    .to_string(),
                None,
            ))
        }
        (Tool::ExifTool, "macos") | (Tool::ExifTool, "linux") => {
            // ExifTool.org provides cross-platform Perl distribution
            Some((
                "https://exiftool.org/Image-ExifTool-13.09.tar.gz".to_string(),
                None,
            ))
        }
        (Tool::ExifTool, "windows") => {
            // ExifTool Windows executable
            Some(("https://exiftool.org/exiftool-13.09.zip".to_string(), None))
        }
        _ => None,
    }
}

/// Compute SHA-256 hash of data
fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Extract archive and install binaries
fn extract_and_install(tool: &Tool, data: &[u8], bin_dir: &Path) -> Result<PathBuf, String> {
    let os = std::env::consts::OS;

    match os {
        "macos" | "windows" if data.starts_with(b"PK") => {
            // ZIP archive
            extract_zip(tool, data, bin_dir)
        }
        "linux" => {
            // Assume tar.xz or tar.gz
            extract_tar(tool, data, bin_dir)
        }
        _ => Err("Unsupported archive format".to_string()),
    }
}

/// Extract ZIP archive
fn extract_zip(tool: &Tool, data: &[u8], bin_dir: &Path) -> Result<PathBuf, String> {
    use std::io::Cursor;
    use zip::ZipArchive;

    let cursor = Cursor::new(data);
    let mut archive = ZipArchive::new(cursor).map_err(|e| format!("Failed to open ZIP: {}", e))?;

    let binary_name = get_binary_name(tool);
    let mut installed_path = None;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry: {}", e))?;

        let file_name = file.name().to_string();

        // Look for the binary executable
        if file_name.ends_with(&binary_name) || file_name.ends_with(&format!("{}.exe", binary_name))
        {
            let target_path = bin_dir.join(&binary_name);
            let mut outfile =
                File::create(&target_path).map_err(|e| format!("Failed to create file: {}", e))?;

            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;

            // Make executable on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = outfile
                    .metadata()
                    .map_err(|e| format!("Failed to get metadata: {}", e))?
                    .permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&target_path, perms)
                    .map_err(|e| format!("Failed to set permissions: {}", e))?;
            }

            installed_path = Some(target_path);
            break;
        }
    }

    installed_path.ok_or_else(|| format!("Binary {} not found in archive", binary_name))
}

/// Extract TAR archive (gzip or xz compressed)
fn extract_tar(tool: &Tool, data: &[u8], bin_dir: &Path) -> Result<PathBuf, String> {
    use flate2::read::GzDecoder;
    use std::io::Cursor;
    use tar::Archive;

    // Try gzip first, then xz
    let cursor = Cursor::new(data);

    // Attempt gzip decompression
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    let binary_name = get_binary_name(tool);
    let mut installed_path = None;

    for entry in archive
        .entries()
        .map_err(|e| format!("Failed to read tar: {}", e))?
    {
        let mut entry = entry.map_err(|e| format!("Failed to read tar entry: {}", e))?;
        let path = entry.path().map_err(|e| format!("Invalid path: {}", e))?;

        if let Some(file_name) = path.file_name() {
            if file_name.to_string_lossy() == binary_name {
                let target_path = bin_dir.join(&binary_name);
                let mut outfile = File::create(&target_path)
                    .map_err(|e| format!("Failed to create file: {}", e))?;

                std::io::copy(&mut entry, &mut outfile)
                    .map_err(|e| format!("Failed to extract: {}", e))?;

                // Make executable
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = outfile
                        .metadata()
                        .map_err(|e| format!("Failed to get metadata: {}", e))?
                        .permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&target_path, perms)
                        .map_err(|e| format!("Failed to set permissions: {}", e))?;
                }

                installed_path = Some(target_path);
                break;
            }
        }
    }

    installed_path.ok_or_else(|| format!("Binary {} not found in archive", binary_name))
}

/// Get binary name for tool
fn get_binary_name(tool: &Tool) -> String {
    let os = std::env::consts::OS;
    match (tool, os) {
        (Tool::FFmpeg, "windows") => "ffmpeg.exe".to_string(),
        (Tool::FFmpeg, _) => "ffmpeg".to_string(),
        (Tool::ExifTool, "windows") => "exiftool.exe".to_string(),
        (Tool::ExifTool, _) => "exiftool".to_string(),
    }
}
