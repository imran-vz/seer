//! Persistent file-based cache for bitrate analysis results
//!
//! Cache location:
//! - macOS/Linux: ~/.cache/seer/bitrate/
//! - Windows: %LOCALAPPDATA%\seer\cache\bitrate\

use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::types::OverallBitrateAnalysis;

/// Maximum number of cache entries to keep
const MAX_CACHE_ENTRIES: usize = 100;

/// Cache entry metadata stored alongside the analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheMetadata {
    pub hash: String,
    pub original_path: String,
    pub cached_at: u64, // Unix timestamp
    pub file_size: u64,
}

/// Combined cache entry (metadata + analysis)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    pub metadata: CacheMetadata,
    pub analysis: OverallBitrateAnalysis,
}

/// Get the cache directory path based on the OS
fn get_cache_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        // Windows: %LOCALAPPDATA%\seer\cache\bitrate
        dirs::data_local_dir().map(|p| p.join("seer").join("cache").join("bitrate"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS/Linux: ~/.cache/seer/bitrate
        dirs::cache_dir().map(|p| p.join("seer").join("bitrate"))
    }
}

/// Ensure the cache directory exists
fn ensure_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = get_cache_dir().ok_or("Could not determine cache directory")?;

    if !cache_dir.exists() {
        fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create cache directory: {}", e))?;
        info!("Created cache directory: {:?}", cache_dir);
    }

    Ok(cache_dir)
}

/// Compute a fast hash for a file based on size, mtime, and sample bytes
/// This is much faster than hashing the entire file
pub fn compute_file_hash(path: &str) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|e| format!("Failed to get metadata: {}", e))?;
    let file_size = metadata.len();
    let mtime = metadata
        .modified()
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        })
        .unwrap_or(0);

    let mut file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;

    // Read first 8KB
    let mut first_bytes = vec![0u8; 8192.min(file_size as usize)];
    file.read_exact(&mut first_bytes)
        .map_err(|e| format!("Failed to read first bytes: {}", e))?;

    // Read last 8KB if file is large enough
    let mut last_bytes = Vec::new();
    if file_size > 16384 {
        file.seek(SeekFrom::End(-8192))
            .map_err(|e| format!("Failed to seek: {}", e))?;
        last_bytes = vec![0u8; 8192];
        file.read_exact(&mut last_bytes)
            .map_err(|e| format!("Failed to read last bytes: {}", e))?;
    }

    // Compute hash
    let mut hasher = Sha256::new();
    hasher.update(file_size.to_le_bytes());
    hasher.update(mtime.to_le_bytes());
    hasher.update(&first_bytes);
    hasher.update(&last_bytes);

    let hash = hasher.finalize();
    Ok(format!("{:x}", hash))
}

/// Get the cache file path for a given hash
fn get_cache_file_path(hash: &str) -> Result<PathBuf, String> {
    let cache_dir = ensure_cache_dir()?;
    // Use first 2 chars as subdirectory to avoid too many files in one dir
    let subdir = &hash[..2.min(hash.len())];
    let subdir_path = cache_dir.join(subdir);

    if !subdir_path.exists() {
        fs::create_dir_all(&subdir_path)
            .map_err(|e| format!("Failed to create cache subdirectory: {}", e))?;
    }

    Ok(subdir_path.join(format!("{}.json", hash)))
}

/// Get cached analysis if available and valid
pub fn get_cached_analysis(path: &str) -> Option<OverallBitrateAnalysis> {
    let hash = match compute_file_hash(path) {
        Ok(h) => h,
        Err(e) => {
            debug!("Failed to compute file hash for cache lookup: {}", e);
            return None;
        }
    };

    let cache_file = match get_cache_file_path(&hash) {
        Ok(p) => p,
        Err(e) => {
            debug!("Failed to get cache file path: {}", e);
            return None;
        }
    };

    if !cache_file.exists() {
        debug!("Cache miss for file: {} (hash: {})", path, &hash[..16]);
        return None;
    }

    // Read and parse cache file
    let content = match fs::read_to_string(&cache_file) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read cache file: {}", e);
            return None;
        }
    };

    let entry: CacheEntry = match serde_json::from_str(&content) {
        Ok(e) => e,
        Err(e) => {
            warn!("Failed to parse cache file (removing corrupt cache): {}", e);
            let _ = fs::remove_file(&cache_file);
            return None;
        }
    };

    // Verify the hash matches
    if entry.metadata.hash != hash {
        debug!("Cache hash mismatch, invalidating");
        let _ = fs::remove_file(&cache_file);
        return None;
    }

    info!(
        "Cache hit for file: {} (hash: {}, cached {} seconds ago)",
        path,
        &hash[..16],
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(entry.metadata.cached_at)
    );

    Some(entry.analysis)
}

/// Save analysis result to cache
pub fn save_to_cache(path: &str, result: &OverallBitrateAnalysis) -> Result<(), String> {
    let hash = compute_file_hash(path)?;
    let cache_file = get_cache_file_path(&hash)?;

    let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let entry = CacheEntry {
        metadata: CacheMetadata {
            hash: hash.clone(),
            original_path: path.to_string(),
            cached_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            file_size,
        },
        analysis: result.clone(),
    };

    let json = serde_json::to_string_pretty(&entry)
        .map_err(|e| format!("Failed to serialize cache entry: {}", e))?;

    let mut file =
        File::create(&cache_file).map_err(|e| format!("Failed to create cache file: {}", e))?;

    file.write_all(json.as_bytes())
        .map_err(|e| format!("Failed to write cache file: {}", e))?;

    info!("Saved analysis to cache: {} (hash: {})", path, &hash[..16]);

    // Cleanup old entries if needed
    if let Err(e) = cleanup_old_entries() {
        warn!("Failed to cleanup old cache entries: {}", e);
    }

    Ok(())
}

/// Clear all cached entries
pub fn clear_cache() -> Result<usize, String> {
    let cache_dir = get_cache_dir().ok_or("Could not determine cache directory")?;

    if !cache_dir.exists() {
        return Ok(0);
    }

    let mut count = 0;

    // Walk through all subdirectories and delete json files
    for entry in fs::read_dir(&cache_dir).map_err(|e| format!("Failed to read cache dir: {}", e))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if path.is_dir() {
            // Delete all json files in subdirectory
            if let Ok(subdir) = fs::read_dir(&path) {
                for subentry in subdir.flatten() {
                    let subpath = subentry.path();
                    if subpath.extension().map(|e| e == "json").unwrap_or(false) {
                        if fs::remove_file(&subpath).is_ok() {
                            count += 1;
                        }
                    }
                }
            }
            // Try to remove empty subdirectory
            let _ = fs::remove_dir(&path);
        } else if path.extension().map(|e| e == "json").unwrap_or(false) {
            if fs::remove_file(&path).is_ok() {
                count += 1;
            }
        }
    }

    info!("Cleared {} cached bitrate analyses", count);
    Ok(count)
}

/// Cleanup old cache entries if we exceed the maximum
fn cleanup_old_entries() -> Result<(), String> {
    let cache_dir = get_cache_dir().ok_or("Could not determine cache directory")?;

    if !cache_dir.exists() {
        return Ok(());
    }

    // Collect all cache entries with their timestamps
    let mut entries: Vec<(PathBuf, u64)> = Vec::new();

    for entry in fs::read_dir(&cache_dir).map_err(|e| format!("Failed to read cache dir: {}", e))? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if path.is_dir() {
            if let Ok(subdir) = fs::read_dir(&path) {
                for subentry in subdir.flatten() {
                    let subpath = subentry.path();
                    if subpath.extension().map(|e| e == "json").unwrap_or(false) {
                        // Try to get cached_at from file content, fall back to mtime
                        let timestamp = fs::read_to_string(&subpath)
                            .ok()
                            .and_then(|c| serde_json::from_str::<CacheEntry>(&c).ok())
                            .map(|e| e.metadata.cached_at)
                            .or_else(|| {
                                fs::metadata(&subpath)
                                    .and_then(|m| m.modified())
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                            })
                            .unwrap_or(0);

                        entries.push((subpath, timestamp));
                    }
                }
            }
        }
    }

    // If under limit, nothing to do
    if entries.len() <= MAX_CACHE_ENTRIES {
        return Ok(());
    }

    // Sort by timestamp (oldest first)
    entries.sort_by_key(|(_, ts)| *ts);

    // Remove oldest entries until we're under the limit
    let to_remove = entries.len() - MAX_CACHE_ENTRIES;
    for (path, _) in entries.into_iter().take(to_remove) {
        if fs::remove_file(&path).is_ok() {
            debug!("Evicted old cache entry: {:?}", path);
        }
    }

    info!("Cleaned up {} old cache entries", to_remove);
    Ok(())
}

/// Get cache statistics
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_size_bytes: u64,
    pub cache_dir: String,
}

#[allow(dead_code)]
pub fn get_cache_stats() -> Result<CacheStats, String> {
    let cache_dir = get_cache_dir().ok_or("Could not determine cache directory")?;

    let mut entry_count = 0;
    let mut total_size: u64 = 0;

    if cache_dir.exists() {
        for entry in fs::read_dir(&cache_dir).map_err(|e| e.to_string())? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();
            if path.is_dir() {
                if let Ok(subdir) = fs::read_dir(&path) {
                    for subentry in subdir.flatten() {
                        let subpath = subentry.path();
                        if subpath.extension().map(|e| e == "json").unwrap_or(false) {
                            entry_count += 1;
                            if let Ok(meta) = fs::metadata(&subpath) {
                                total_size += meta.len();
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(CacheStats {
        entry_count,
        total_size_bytes: total_size,
        cache_dir: cache_dir.to_string_lossy().to_string(),
    })
}
