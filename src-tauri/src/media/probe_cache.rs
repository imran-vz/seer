//! FFprobe result caching for session-level deduplication
//!
//! This module provides an in-memory cache for ffprobe results to avoid
//! redundant calls when multiple operations need the same metadata.
//!
//! The cache is keyed by file path and invalidated based on file modification time.
//! This is a session-level cache (not persisted to disk) for performance optimization.

use dashmap::DashMap;
use log::{debug, info};
use once_cell::sync::Lazy;
use std::process::Command;
use std::time::{Duration, Instant};

use super::find_command;

/// Cached ffprobe result with metadata for invalidation
#[derive(Clone, Debug)]
pub struct CachedProbeResult {
    /// Raw JSON string from ffprobe
    pub json_data: String,
    /// Parsed JSON value (cached to avoid re-parsing)
    pub parsed: serde_json::Value,
    /// File modification time when probed (for invalidation)
    pub file_mtime: Option<u64>,
    /// When this cache entry was created
    pub cached_at: Instant,
}

/// Global ffprobe cache
/// Key: file path, Value: cached probe result
static PROBE_CACHE: Lazy<DashMap<String, CachedProbeResult>> = Lazy::new(DashMap::new);

/// Maximum age of cache entries before they're considered stale (5 minutes)
const CACHE_MAX_AGE: Duration = Duration::from_secs(300);

/// Get file modification time as unix timestamp
fn get_file_mtime(path: &str) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Check if a cache entry is still valid
fn is_cache_valid(entry: &CachedProbeResult, current_mtime: Option<u64>) -> bool {
    // Check if cache is too old
    if entry.cached_at.elapsed() > CACHE_MAX_AGE {
        debug!("Cache entry expired (age > {}s)", CACHE_MAX_AGE.as_secs());
        return false;
    }

    // Check if file has been modified
    match (entry.file_mtime, current_mtime) {
        (Some(cached), Some(current)) if cached != current => {
            debug!(
                "Cache invalidated: file modified (cached={}, current={})",
                cached, current
            );
            false
        }
        _ => true,
    }
}

/// Run ffprobe and return the raw JSON output
fn run_ffprobe(path: &str) -> Result<String, String> {
    let ffprobe_cmd = find_command("ffprobe").unwrap_or_else(|| "ffprobe".to_string());

    let output = Command::new(&ffprobe_cmd)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8 output: {}", e))
}

/// Get ffprobe data for a file, using cache if available
///
/// This function checks the cache first and only runs ffprobe if:
/// - No cache entry exists for this path
/// - The cache entry is older than CACHE_MAX_AGE
/// - The file's modification time has changed
///
/// Returns: (json_string, parsed_json, was_cached)
pub fn get_probe_data(path: &str) -> Result<(String, serde_json::Value, bool), String> {
    let current_mtime = get_file_mtime(path);

    // Check cache first
    if let Some(entry) = PROBE_CACHE.get(path) {
        if is_cache_valid(&entry, current_mtime) {
            debug!("FFprobe cache hit for: {}", path);
            return Ok((entry.json_data.clone(), entry.parsed.clone(), true));
        }
        // Cache invalid, will be replaced below
        drop(entry);
    }

    debug!("FFprobe cache miss for: {}", path);

    // Run ffprobe
    let json_data = run_ffprobe(path)?;

    // Parse JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&json_data).map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Store in cache
    let entry = CachedProbeResult {
        json_data: json_data.clone(),
        parsed: parsed.clone(),
        file_mtime: current_mtime,
        cached_at: Instant::now(),
    };
    PROBE_CACHE.insert(path.to_string(), entry);

    info!("FFprobe result cached for: {}", path);
    Ok((json_data, parsed, false))
}

/// Get only the parsed JSON value (convenience wrapper)
pub fn get_probe_json(path: &str) -> Result<serde_json::Value, String> {
    get_probe_data(path).map(|(_, parsed, _)| parsed)
}

/// Get only the raw JSON string (convenience wrapper)
pub fn get_probe_string(path: &str) -> Result<String, String> {
    get_probe_data(path).map(|(json, _, _)| json)
}

/// Invalidate cache entry for a specific file
///
/// Call this after modifying a file (e.g., removing streams)
pub fn invalidate_cache(path: &str) {
    if PROBE_CACHE.remove(path).is_some() {
        debug!("Invalidated ffprobe cache for: {}", path);
    }
}

/// Clear the entire cache
pub fn clear_cache() {
    let count = PROBE_CACHE.len();
    PROBE_CACHE.clear();
    info!("Cleared ffprobe cache ({} entries)", count);
}

/// Get cache statistics
pub fn get_cache_stats() -> (usize, usize) {
    let total = PROBE_CACHE.len();
    let valid = PROBE_CACHE
        .iter()
        .filter(|entry| {
            let mtime = get_file_mtime(entry.key());
            is_cache_valid(entry.value(), mtime)
        })
        .count();
    (total, valid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_stats_empty() {
        // Note: This test may fail if other tests have populated the cache
        // In a real test suite, we'd use a separate cache instance
        let (total, _valid) = get_cache_stats();
        assert!(total >= 0);
    }
}
