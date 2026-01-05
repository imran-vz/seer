/**
 * File Metadata Caching Module
 *
 * Provides caching for file metadata in SQLite with hash-based validation
 * to ensure cached data stays in sync with the actual file on disk.
 *
 * Uses the file hash (based on size, mtime, and content samples) combined
 * with the file path as a lookup index to detect when files have changed.
 */

import { invoke } from "@tauri-apps/api/core";
import type { CacheType } from "@/types/database";
import {
	deleteCacheByFilePath,
	generateCacheKey,
	getValidCache,
	setCache,
} from "./database";

// ============================================================================
// Types
// ============================================================================

export interface FileMetadata {
	path: string;
	name: string;
	size: number;
	modified: string | null;
	created: string | null;
	is_media: boolean;
	extension: string | null;
	ffprobe_data: string | null;
}

export interface CachedFileMetadata extends FileMetadata {
	from_cache: boolean;
	cached_at?: string;
}

export interface FileMetadataCacheOptions {
	/** Time-to-live in seconds (default: 7 days) */
	ttlSeconds?: number;
	/** Whether to skip cache and always fetch fresh data */
	skipCache?: boolean;
	/** Whether to force refresh the cache even if valid */
	forceRefresh?: boolean;
}

// ============================================================================
// Constants
// ============================================================================

const CACHE_TYPE: CacheType = "file_metadata";
const DEFAULT_TTL_SECONDS = 60 * 60 * 24 * 7; // 7 days

// ============================================================================
// Helper Functions
// ============================================================================

/**
 * Compute a file hash using the backend command
 * This hash is based on file size, modification time, and content samples
 */
export async function computeFileHash(filePath: string): Promise<string> {
	return invoke<string>("compute_file_hash_cmd", { path: filePath });
}

// ============================================================================
// Main Cache Functions
// ============================================================================

/**
 * Get file metadata with caching
 *
 * This function:
 * 1. Computes the current file hash
 * 2. Checks the SQLite cache for existing metadata
 * 3. Validates the cached hash matches the current hash
 * 4. Returns cached data if valid, otherwise fetches fresh data
 * 5. Stores fresh data in the cache for future use
 *
 * @param filePath - The path to the file
 * @param options - Cache options
 * @returns The file metadata (with from_cache flag indicating source)
 */
export async function getFileMetadataCached(
	filePath: string,
	options: FileMetadataCacheOptions = {},
): Promise<CachedFileMetadata> {
	const {
		ttlSeconds = DEFAULT_TTL_SECONDS,
		skipCache = false,
		forceRefresh = false,
	} = options;

	const cacheKey = generateCacheKey(CACHE_TYPE, filePath);

	// If not skipping cache and not forcing refresh, try to get from cache
	if (!skipCache && !forceRefresh) {
		try {
			// Compute current file hash for validation
			const currentHash = await computeFileHash(filePath);

			// Try to get valid cached data
			const cached = await getValidCache(cacheKey, currentHash);

			if (cached) {
				console.log(`[FileMetadataCache] Cache hit for ${filePath}`);
				const data = cached.data as unknown as FileMetadata;
				return {
					...data,
					from_cache: true,
					cached_at: cached.created_at,
				};
			}

			console.log(`[FileMetadataCache] Cache miss for ${filePath}`);
		} catch (err) {
			// Log error but continue to fetch fresh data
			console.warn(`[FileMetadataCache] Error checking cache:`, err);
		}
	}

	// Fetch fresh metadata from backend
	console.log(`[FileMetadataCache] Fetching fresh metadata for ${filePath}`);
	const metadata = await invoke<FileMetadata>("get_file_metadata", {
		path: filePath,
	});

	// Cache the result
	if (!skipCache) {
		try {
			const fileHash = await computeFileHash(filePath);

			await setCache({
				cache_type: CACHE_TYPE,
				cache_key: cacheKey,
				file_path: filePath,
				file_hash: fileHash,
				file_size: metadata.size,
				file_modified_at: metadata.modified || new Date().toISOString(),
				data: {
					path: metadata.path,
					name: metadata.name,
					size: metadata.size,
					modified: metadata.modified,
					created: metadata.created,
					is_media: metadata.is_media,
					extension: metadata.extension,
					ffprobe_data: metadata.ffprobe_data,
				},
				ttl_seconds: ttlSeconds,
			});

			console.log(`[FileMetadataCache] Cached metadata for ${filePath}`);
		} catch (err) {
			// Log error but don't fail - we still have the fresh data
			console.warn(`[FileMetadataCache] Error caching metadata:`, err);
		}
	}

	return {
		...metadata,
		from_cache: false,
	};
}

/**
 * Invalidate cached metadata for a file
 * Call this when you know a file has been modified
 *
 * @param filePath - The path to the file
 * @returns The number of cache entries deleted
 */
export async function invalidateFileMetadata(
	filePath: string,
): Promise<number> {
	console.log(`[FileMetadataCache] Invalidating cache for ${filePath}`);
	return deleteCacheByFilePath(filePath);
}

/**
 * Prefetch and cache metadata for multiple files
 * Useful for pre-warming the cache when browsing directories
 *
 * @param filePaths - Array of file paths to prefetch
 * @param options - Cache options
 * @returns Map of file paths to their metadata (or errors)
 */
export async function prefetchFileMetadata(
	filePaths: string[],
	options: FileMetadataCacheOptions = {},
): Promise<Map<string, CachedFileMetadata | Error>> {
	const results = new Map<string, CachedFileMetadata | Error>();

	// Process in parallel with a concurrency limit
	const CONCURRENCY = 5;
	const chunks: string[][] = [];

	for (let i = 0; i < filePaths.length; i += CONCURRENCY) {
		chunks.push(filePaths.slice(i, i + CONCURRENCY));
	}

	for (const chunk of chunks) {
		const promises = chunk.map(async (filePath) => {
			try {
				const metadata = await getFileMetadataCached(filePath, options);
				results.set(filePath, metadata);
			} catch (err) {
				results.set(
					filePath,
					err instanceof Error ? err : new Error(String(err)),
				);
			}
		});

		await Promise.all(promises);
	}

	return results;
}

/**
 * Check if a file's cached metadata is still valid
 * Returns true if cache exists and file hash matches
 *
 * @param filePath - The path to the file
 * @returns Whether the cache is valid
 */
export async function isMetadataCacheValid(filePath: string): Promise<boolean> {
	try {
		const cacheKey = generateCacheKey(CACHE_TYPE, filePath);
		const currentHash = await computeFileHash(filePath);
		const cached = await getValidCache(cacheKey, currentHash);
		return cached !== null;
	} catch {
		return false;
	}
}

/**
 * Get the cache key for a file (useful for debugging)
 *
 * @param filePath - The path to the file
 * @returns The cache key string
 */
export function getMetadataCacheKey(filePath: string): string {
	return generateCacheKey(CACHE_TYPE, filePath);
}
