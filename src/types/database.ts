/**
 * Database types for SQLite storage
 * Used for job tracking and caching
 */

// ============================================================================
// Job Types
// ============================================================================

export type JobType =
	| "bitrate_analysis"
	| "stream_removal"
	| "reencode"
	| "metadata_edit";

export type DbJobStatus =
	| "pending"
	| "running"
	| "completed"
	| "failed"
	| "cancelled";

export interface Job {
	id: number;
	job_type: JobType;
	status: DbJobStatus;
	file_path: string;
	file_hash: string | null;
	params: string | null; // JSON-encoded parameters
	result: string | null; // JSON-encoded result
	error_message: string | null;
	progress: number; // 0-100
	created_at: string; // ISO timestamp
	started_at: string | null;
	completed_at: string | null;
}

export interface CreateJobParams {
	job_type: JobType;
	file_path: string;
	file_hash?: string;
	params?: Record<string, unknown>;
}

export interface UpdateJobParams {
	status?: DbJobStatus;
	progress?: number;
	result?: Record<string, unknown>;
	error_message?: string;
	started_at?: string;
	completed_at?: string;
}

// ============================================================================
// Cache Types
// ============================================================================

export type CacheType =
	| "bitrate_analysis"
	| "media_streams"
	| "file_metadata"
	| "ffprobe_data";

export interface CacheEntry {
	id: number;
	cache_type: CacheType;
	cache_key: string; // Usually file_path + optional params hash
	file_path: string;
	file_hash: string; // SHA-256 hash for cache invalidation
	file_size: number;
	file_modified_at: string;
	data: string; // JSON-encoded cached data
	created_at: string;
	expires_at: string | null;
}

export interface CreateCacheParams {
	cache_type: CacheType;
	cache_key: string;
	file_path: string;
	file_hash: string;
	file_size: number;
	file_modified_at: string;
	data: Record<string, unknown>;
	ttl_seconds?: number; // Time-to-live in seconds
}

// ============================================================================
// Query Result Types
// ============================================================================

export interface JobWithParsedData extends Omit<Job, "params" | "result"> {
	params: Record<string, unknown> | null;
	result: Record<string, unknown> | null;
}

export interface CacheWithParsedData extends Omit<CacheEntry, "data"> {
	data: Record<string, unknown>;
}

// ============================================================================
// Statistics Types
// ============================================================================

export interface JobStatistics {
	total_jobs: number;
	pending_jobs: number;
	running_jobs: number;
	completed_jobs: number;
	failed_jobs: number;
	cancelled_jobs: number;
}

export interface CacheStatistics {
	total_entries: number;
	total_size_bytes: number;
	entries_by_type: Record<CacheType, number>;
	oldest_entry: string | null;
	newest_entry: string | null;
}

// ============================================================================
// Database Configuration
// ============================================================================

export const DATABASE_NAME = "seer.db";

export const CACHE_DEFAULT_TTL_SECONDS = 60 * 60 * 24 * 7; // 7 days
