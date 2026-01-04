//! Bitrate analysis cache module
//!
//! This module previously handled file-based caching for bitrate analysis results.
//! Caching is now handled by the frontend via SQLite database for better consistency
//! and to centralize all data storage in one place.
//!
//! The functions here are kept as stubs to maintain API compatibility with the
//! commands module, but they now delegate caching responsibility to the frontend.

use crate::types::OverallBitrateAnalysis;

/// Get cached analysis - always returns None as caching is now handled by frontend
///
/// The frontend checks the SQLite database for cached analysis before calling
/// the backend analysis functions. This stub is kept for API compatibility.
pub fn get_cached_analysis(_path: &str) -> Option<OverallBitrateAnalysis> {
    // Caching is now handled by the frontend via SQLite database
    // See: src/lib/database.ts - getBitrateAnalysis()
    None
}

/// Save analysis to cache - no-op as caching is now handled by frontend
///
/// The frontend saves analysis results to SQLite after receiving them from
/// the backend. This stub is kept for API compatibility.
pub fn save_to_cache(_path: &str, _result: &OverallBitrateAnalysis) -> Result<(), String> {
    // Caching is now handled by the frontend via SQLite database
    // See: src/lib/database.ts - saveBitrateAnalysis()
    Ok(())
}

/// Clear cache - returns 0 as file-based cache is no longer used
///
/// Cache clearing is now handled via the frontend's database functions.
/// See: Settings dialog -> Clear Bitrate Data
pub fn clear_cache() -> Result<usize, String> {
    // File-based cache has been removed
    // Database cache is cleared via frontend: clearAllBitrateAnalysis()
    Ok(0)
}

/// Compute a fast hash for a file based on size, mtime, and sample bytes
///
/// This function is still useful for cache validation - the frontend uses
/// this hash to verify if cached data is still valid for the current file.
pub fn compute_file_hash(path: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::fs::{self, File};
    use std::io::{Read, Seek, SeekFrom};

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
