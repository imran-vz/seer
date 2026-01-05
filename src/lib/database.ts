/**
 * Database service for SQLite operations
 * Handles job tracking, caching, and bitrate analysis storage using tauri-plugin-sql
 */

import Database from "@tauri-apps/plugin-sql";
import type {
	BitrateDataPoint,
	BitrateStatistics,
	OverallBitrateAnalysis,
	StreamContribution,
} from "@/types/bitrate";
import {
	CACHE_DEFAULT_TTL_SECONDS,
	type CacheEntry,
	type CacheStatistics,
	type CacheType,
	type CacheWithParsedData,
	type CreateCacheParams,
	type CreateJobParams,
	DATABASE_NAME,
	type DbJobStatus,
	type Job,
	type JobStatistics,
	type JobType,
	type JobWithParsedData,
	type UpdateJobParams,
} from "@/types/database";

// ============================================================================
// Database Instance
// ============================================================================

let db: Database | null = null;
let dbPromise: Promise<Database> | null = null;

/**
 * Get the database instance, initializing if necessary
 * Uses promise-based singleton to prevent race conditions
 * Note: Schema is created by Tauri migrations in Rust
 */
export async function getDatabase(): Promise<Database> {
	// Return existing connection
	if (db) return db;

	// Return in-progress initialization promise to prevent race conditions
	if (dbPromise) return dbPromise;

	// Start initialization and store the promise
	dbPromise = Database.load(`sqlite:${DATABASE_NAME}`).then((database) => {
		db = database;
		console.log("[Database] Connected to SQLite database");
		return database;
	});

	return dbPromise;
}

/**
 * Close the database connection
 */
export async function closeDatabase(): Promise<void> {
	if (db) {
		await db.close();
		db = null;
		console.log("[Database] Connection closed");
	}
}

// ============================================================================
// Job Operations
// ============================================================================

/**
 * Create a new job
 */
export async function createJob(params: CreateJobParams): Promise<Job> {
	const database = await getDatabase();

	const result = await database.execute(
		`INSERT INTO jobs (job_type, file_path, file_hash, params)
		 VALUES ($1, $2, $3, $4)`,
		[
			params.job_type,
			params.file_path,
			params.file_hash ?? null,
			params.params ? JSON.stringify(params.params) : null,
		],
	);

	if (result.lastInsertId === undefined) {
		throw new Error("Failed to get last insert ID");
	}

	const job = await getJobById(result.lastInsertId);
	if (!job) throw new Error("Failed to create job");

	console.log(`[Database] Created job ${job.id}: ${job.job_type}`);
	return job;
}

/**
 * Get a job by ID
 */
export async function getJobById(id: number): Promise<Job | null> {
	const database = await getDatabase();

	const rows = await database.select<Job[]>(
		"SELECT * FROM jobs WHERE id = $1",
		[id],
	);

	return rows[0] ?? null;
}

/**
 * Get a job by ID with parsed JSON fields
 */
export async function getJobByIdParsed(
	id: number,
): Promise<JobWithParsedData | null> {
	const job = await getJobById(id);
	if (!job) return null;

	return {
		...job,
		params: job.params ? JSON.parse(job.params) : null,
		result: job.result ? JSON.parse(job.result) : null,
	};
}

/**
 * Update a job
 */
export async function updateJob(
	id: number,
	updates: UpdateJobParams,
): Promise<Job | null> {
	const database = await getDatabase();

	const setClauses: string[] = [];
	const values: unknown[] = [];
	let paramIndex = 1;

	if (updates.status !== undefined) {
		setClauses.push(`status = $${paramIndex++}`);
		values.push(updates.status);
	}
	if (updates.progress !== undefined) {
		setClauses.push(`progress = $${paramIndex++}`);
		values.push(updates.progress);
	}
	if (updates.result !== undefined) {
		setClauses.push(`result = $${paramIndex++}`);
		values.push(JSON.stringify(updates.result));
	}
	if (updates.error_message !== undefined) {
		setClauses.push(`error_message = $${paramIndex++}`);
		values.push(updates.error_message);
	}
	if (updates.started_at !== undefined) {
		setClauses.push(`started_at = $${paramIndex++}`);
		values.push(updates.started_at);
	}
	if (updates.completed_at !== undefined) {
		setClauses.push(`completed_at = $${paramIndex++}`);
		values.push(updates.completed_at);
	}

	if (setClauses.length === 0) {
		return getJobById(id);
	}

	values.push(id);
	await database.execute(
		`UPDATE jobs SET ${setClauses.join(", ")} WHERE id = $${paramIndex}`,
		values,
	);

	return getJobById(id);
}

/**
 * Start a job (set status to running and record start time)
 */
export async function startJob(id: number): Promise<Job | null> {
	return updateJob(id, {
		status: "running",
		started_at: new Date().toISOString(),
	});
}

/**
 * Complete a job successfully
 */
export async function completeJob(
	id: number,
	result?: Record<string, unknown>,
): Promise<Job | null> {
	return updateJob(id, {
		status: "completed",
		progress: 100,
		result,
		completed_at: new Date().toISOString(),
	});
}

/**
 * Fail a job with an error message
 */
export async function failJob(
	id: number,
	errorMessage: string,
): Promise<Job | null> {
	return updateJob(id, {
		status: "failed",
		error_message: errorMessage,
		completed_at: new Date().toISOString(),
	});
}

/**
 * Cancel a job
 */
export async function cancelJob(id: number): Promise<Job | null> {
	return updateJob(id, {
		status: "cancelled",
		completed_at: new Date().toISOString(),
	});
}

/**
 * Get jobs by status
 */
export async function getJobsByStatus(status: DbJobStatus): Promise<Job[]> {
	const database = await getDatabase();

	return database.select<Job[]>(
		"SELECT * FROM jobs WHERE status = $1 ORDER BY created_at DESC",
		[status],
	);
}

/**
 * Get jobs by file path
 */
export async function getJobsByFilePath(filePath: string): Promise<Job[]> {
	const database = await getDatabase();

	return database.select<Job[]>(
		"SELECT * FROM jobs WHERE file_path = $1 ORDER BY created_at DESC",
		[filePath],
	);
}

/**
 * Get jobs by type
 */
export async function getJobsByType(jobType: JobType): Promise<Job[]> {
	const database = await getDatabase();

	return database.select<Job[]>(
		"SELECT * FROM jobs WHERE job_type = $1 ORDER BY created_at DESC",
		[jobType],
	);
}

/**
 * Get all running jobs
 */
export async function getRunningJobs(): Promise<Job[]> {
	return getJobsByStatus("running");
}

/**
 * Get all pending jobs
 */
export async function getPendingJobs(): Promise<Job[]> {
	return getJobsByStatus("pending");
}

/**
 * Get recent jobs (last N jobs)
 */
export async function getRecentJobs(limit = 50): Promise<Job[]> {
	const database = await getDatabase();

	return database.select<Job[]>(
		"SELECT * FROM jobs ORDER BY created_at DESC LIMIT $1",
		[limit],
	);
}

/**
 * Delete old completed/failed/cancelled jobs
 */
export async function cleanupOldJobs(daysOld = 30): Promise<number> {
	const database = await getDatabase();

	const result = await database.execute(
		`DELETE FROM jobs
		 WHERE status IN ('completed', 'failed', 'cancelled')
		 AND datetime(completed_at) < datetime('now', $1)`,
		[`-${daysOld} days`],
	);

	console.log(`[Database] Cleaned up ${result.rowsAffected} old jobs`);
	return result.rowsAffected;
}

/**
 * Clear all jobs from the database
 */
export async function clearAllJobs(): Promise<number> {
	const database = await getDatabase();

	const result = await database.execute("DELETE FROM jobs");

	console.log(`[Database] Cleared ${result.rowsAffected} jobs`);
	return result.rowsAffected;
}

/**
 * Get job statistics
 */
export async function getJobStatistics(): Promise<JobStatistics> {
	const database = await getDatabase();

	const rows = await database.select<{ status: DbJobStatus; count: number }[]>(
		`SELECT status, COUNT(*) as count FROM jobs GROUP BY status`,
	);

	const stats: JobStatistics = {
		total_jobs: 0,
		pending_jobs: 0,
		running_jobs: 0,
		completed_jobs: 0,
		failed_jobs: 0,
		cancelled_jobs: 0,
	};

	for (const row of rows) {
		stats.total_jobs += row.count;
		switch (row.status) {
			case "pending":
				stats.pending_jobs = row.count;
				break;
			case "running":
				stats.running_jobs = row.count;
				break;
			case "completed":
				stats.completed_jobs = row.count;
				break;
			case "failed":
				stats.failed_jobs = row.count;
				break;
			case "cancelled":
				stats.cancelled_jobs = row.count;
				break;
		}
	}

	return stats;
}

// ============================================================================
// Cache Operations
// ============================================================================

/**
 * Generate a cache key for a file and optional parameters
 */
export function generateCacheKey(
	cacheType: CacheType,
	filePath: string,
	params?: Record<string, unknown>,
): string {
	const base = `${cacheType}:${filePath}`;
	if (params) {
		const paramsStr = JSON.stringify(params, Object.keys(params).sort());
		return `${base}:${paramsStr}`;
	}
	return base;
}

/**
 * Set a cache entry
 */
export async function setCache(params: CreateCacheParams): Promise<CacheEntry> {
	const database = await getDatabase();

	const expiresAt = params.ttl_seconds
		? new Date(Date.now() + params.ttl_seconds * 1000).toISOString()
		: new Date(Date.now() + CACHE_DEFAULT_TTL_SECONDS * 1000).toISOString();

	// Use INSERT OR REPLACE to handle updates
	await database.execute(
		`INSERT OR REPLACE INTO cache
		 (cache_type, cache_key, file_path, file_hash, file_size, file_modified_at, data, expires_at)
		 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)`,
		[
			params.cache_type,
			params.cache_key,
			params.file_path,
			params.file_hash,
			params.file_size,
			params.file_modified_at,
			JSON.stringify(params.data),
			expiresAt,
		],
	);

	const entry = await getCacheByKey(params.cache_key);
	if (!entry) throw new Error("Failed to create cache entry");

	console.log(`[Database] Cached ${params.cache_type} for ${params.file_path}`);
	return entry;
}

/**
 * Get a cache entry by key
 */
export async function getCacheByKey(
	cacheKey: string,
): Promise<CacheEntry | null> {
	const database = await getDatabase();

	const rows = await database.select<CacheEntry[]>(
		"SELECT * FROM cache WHERE cache_key = $1",
		[cacheKey],
	);

	return rows[0] ?? null;
}

/**
 * Get a cache entry by key with parsed data
 */
export async function getCacheByKeyParsed(
	cacheKey: string,
): Promise<CacheWithParsedData | null> {
	const entry = await getCacheByKey(cacheKey);
	if (!entry) return null;

	return {
		...entry,
		data: JSON.parse(entry.data),
	};
}

/**
 * Get a valid (non-expired) cache entry
 * Also validates that the file hasn't changed based on hash
 */
export async function getValidCache(
	cacheKey: string,
	currentFileHash?: string,
): Promise<CacheWithParsedData | null> {
	const database = await getDatabase();

	// Check if entry exists and hasn't expired
	const rows = await database.select<CacheEntry[]>(
		`SELECT * FROM cache
		 WHERE cache_key = $1
		 AND (expires_at IS NULL OR datetime(expires_at) > datetime('now'))`,
		[cacheKey],
	);

	const entry = rows[0];
	if (!entry) return null;

	// If a current file hash is provided, validate it matches
	if (currentFileHash && entry.file_hash !== currentFileHash) {
		console.log(
			`[Database] Cache invalidated for ${cacheKey}: file hash changed`,
		);
		await deleteCache(cacheKey);
		return null;
	}

	return {
		...entry,
		data: JSON.parse(entry.data),
	};
}

/**
 * Get all cache entries for a file path
 */
export async function getCacheByFilePath(
	filePath: string,
): Promise<CacheEntry[]> {
	const database = await getDatabase();

	return database.select<CacheEntry[]>(
		"SELECT * FROM cache WHERE file_path = $1 ORDER BY created_at DESC",
		[filePath],
	);
}

/**
 * Get cache entries by type
 */
export async function getCacheByType(
	cacheType: CacheType,
): Promise<CacheEntry[]> {
	const database = await getDatabase();

	return database.select<CacheEntry[]>(
		"SELECT * FROM cache WHERE cache_type = $1 ORDER BY created_at DESC",
		[cacheType],
	);
}

/**
 * Delete a cache entry by key
 */
export async function deleteCache(cacheKey: string): Promise<boolean> {
	const database = await getDatabase();

	const result = await database.execute(
		"DELETE FROM cache WHERE cache_key = $1",
		[cacheKey],
	);

	return result.rowsAffected > 0;
}

/**
 * Delete all cache entries for a file path
 */
export async function deleteCacheByFilePath(filePath: string): Promise<number> {
	const database = await getDatabase();

	const result = await database.execute(
		"DELETE FROM cache WHERE file_path = $1",
		[filePath],
	);

	console.log(
		`[Database] Deleted ${result.rowsAffected} cache entries for ${filePath}`,
	);
	return result.rowsAffected;
}

/**
 * Delete all cache entries of a specific type
 */
export async function deleteCacheByType(cacheType: CacheType): Promise<number> {
	const database = await getDatabase();

	const result = await database.execute(
		"DELETE FROM cache WHERE cache_type = $1",
		[cacheType],
	);

	console.log(
		`[Database] Deleted ${result.rowsAffected} cache entries of type ${cacheType}`,
	);
	return result.rowsAffected;
}

/**
 * Delete expired cache entries
 */
export async function cleanupExpiredCache(): Promise<number> {
	const database = await getDatabase();

	const result = await database.execute(
		`DELETE FROM cache WHERE expires_at IS NOT NULL AND datetime(expires_at) < datetime('now')`,
	);

	console.log(
		`[Database] Cleaned up ${result.rowsAffected} expired cache entries`,
	);
	return result.rowsAffected;
}

/**
 * Clear all cache entries
 */
export async function clearAllCache(): Promise<number> {
	const database = await getDatabase();

	const result = await database.execute("DELETE FROM cache");

	console.log(`[Database] Cleared ${result.rowsAffected} cache entries`);
	return result.rowsAffected;
}

/**
 * Get cache statistics
 */
export async function getCacheStatistics(): Promise<CacheStatistics> {
	const database = await getDatabase();

	// Get total count
	const countRows = await database.select<{ count: number }[]>(
		"SELECT COUNT(*) as count FROM cache",
	);

	// Get total size (approximate based on data field length)
	const sizeRows = await database.select<{ total_size: number | null }[]>(
		"SELECT SUM(LENGTH(data)) as total_size FROM cache",
	);

	// Get counts by type
	const typeRows = await database.select<
		{ cache_type: CacheType; count: number }[]
	>("SELECT cache_type, COUNT(*) as count FROM cache GROUP BY cache_type");

	// Get oldest and newest entries
	const oldestRows = await database.select<{ created_at: string }[]>(
		"SELECT created_at FROM cache ORDER BY created_at ASC LIMIT 1",
	);
	const newestRows = await database.select<{ created_at: string }[]>(
		"SELECT created_at FROM cache ORDER BY created_at DESC LIMIT 1",
	);

	const entriesByType: Record<CacheType, number> = {
		bitrate_analysis: 0,
		media_streams: 0,
		file_metadata: 0,
		ffprobe_data: 0,
	};

	for (const row of typeRows) {
		entriesByType[row.cache_type] = row.count;
	}

	return {
		total_entries: countRows[0]?.count ?? 0,
		total_size_bytes: sizeRows[0]?.total_size ?? 0,
		entries_by_type: entriesByType,
		oldest_entry: oldestRows[0]?.created_at ?? null,
		newest_entry: newestRows[0]?.created_at ?? null,
	};
}

// ============================================================================
// Bitrate Analysis Storage
// ============================================================================

interface BitrateAnalysisRow {
	id: number;
	file_path: string;
	file_hash: string;
	file_size: number;
	duration: number;
	interval_seconds: number;
	created_at: string;
}

interface BitrateStatisticsRow {
	id: number;
	analysis_id: number;
	min_bitrate: number;
	max_bitrate: number;
	avg_bitrate: number;
	median_bitrate: number;
	std_deviation: number;
	total_frames: number;
}

interface PeakIntervalRow {
	id: number;
	statistics_id: number;
	start_time: number;
	end_time: number;
	peak_bitrate: number;
	duration: number;
}

interface StreamContributionRow {
	id: number;
	analysis_id: number;
	stream_index: number;
	stream_type: string;
	codec_name: string;
	percentage: number;
}

interface DataPointRow {
	id: number;
	analysis_id?: number;
	contribution_id?: number;
	timestamp: number;
	bitrate: number;
	frame_type: string | null;
}

/**
 * Save a complete bitrate analysis to the database
 */
export async function saveBitrateAnalysis(
	analysis: OverallBitrateAnalysis,
	fileHash: string,
	fileSize: number,
	intervalSeconds: number,
): Promise<number> {
	const database = await getDatabase();

	// Delete any existing analysis for this file/hash combo
	await deleteBitrateAnalysisByPath(analysis.path);

	// Insert main analysis record
	const analysisResult = await database.execute(
		`INSERT INTO bitrate_analysis (file_path, file_hash, file_size, duration, interval_seconds)
		 VALUES ($1, $2, $3, $4, $5)`,
		[analysis.path, fileHash, fileSize, analysis.duration, intervalSeconds],
	);

	if (analysisResult.lastInsertId === undefined) {
		throw new Error("Failed to insert bitrate analysis");
	}

	const analysisId = analysisResult.lastInsertId;

	// Insert data points in batches for better performance
	await insertDataPointsBatch(database, analysisId, analysis.data_points);

	// Insert statistics
	const statsResult = await database.execute(
		`INSERT INTO bitrate_statistics
		 (analysis_id, min_bitrate, max_bitrate, avg_bitrate, median_bitrate, std_deviation, total_frames)
		 VALUES ($1, $2, $3, $4, $5, $6, $7)`,
		[
			analysisId,
			analysis.statistics.min_bitrate,
			analysis.statistics.max_bitrate,
			analysis.statistics.avg_bitrate,
			analysis.statistics.median_bitrate,
			analysis.statistics.std_deviation,
			analysis.statistics.total_frames,
		],
	);

	if (statsResult.lastInsertId === undefined) {
		throw new Error("Failed to insert bitrate statistics");
	}

	const statsId = statsResult.lastInsertId;

	// Insert peak intervals
	for (const peak of analysis.statistics.peak_intervals) {
		await database.execute(
			`INSERT INTO peak_intervals (statistics_id, start_time, end_time, peak_bitrate, duration)
			 VALUES ($1, $2, $3, $4, $5)`,
			[
				statsId,
				peak.start_time,
				peak.end_time,
				peak.peak_bitrate,
				peak.duration,
			],
		);
	}

	// Insert stream contributions and their data points
	for (const contribution of analysis.stream_contributions) {
		const contribResult = await database.execute(
			`INSERT INTO stream_contributions (analysis_id, stream_index, stream_type, codec_name, percentage)
			 VALUES ($1, $2, $3, $4, $5)`,
			[
				analysisId,
				contribution.stream_index,
				contribution.stream_type,
				contribution.codec_name,
				contribution.percentage,
			],
		);

		if (contribResult.lastInsertId !== undefined) {
			await insertStreamDataPointsBatch(
				database,
				contribResult.lastInsertId,
				contribution.data_points,
			);
		}
	}

	console.log(
		`[Database] Saved bitrate analysis for ${analysis.path} (id: ${analysisId})`,
	);
	return analysisId;
}

/**
 * Insert data points in batches
 */
async function insertDataPointsBatch(
	database: Database,
	analysisId: number,
	dataPoints: BitrateDataPoint[],
): Promise<void> {
	const BATCH_SIZE = 500;
	const FIELDS_PER_ROW = 4;

	for (let i = 0; i < dataPoints.length; i += BATCH_SIZE) {
		const batch = dataPoints.slice(i, i + BATCH_SIZE);
		const placeholders: string[] = [];
		const values: unknown[] = [];

		batch.forEach((point, idx) => {
			const offset = idx * FIELDS_PER_ROW;
			placeholders.push(
				`($${offset + 1}, $${offset + 2}, $${offset + 3}, $${offset + 4})`,
			);
			values.push(
				analysisId,
				point.timestamp,
				point.bitrate,
				point.frame_type ?? null,
			);
		});

		// Validate placeholder count matches values count
		const expectedParams = batch.length * FIELDS_PER_ROW;
		if (values.length !== expectedParams) {
			throw new Error(
				`Batch insert validation failed: expected ${expectedParams} values, got ${values.length}`,
			);
		}

		await database.execute(
			`INSERT INTO bitrate_data_points (analysis_id, timestamp, bitrate, frame_type)
			 VALUES ${placeholders.join(", ")}`,
			values,
		);
	}
}

/**
 * Insert stream data points in batches
 */
async function insertStreamDataPointsBatch(
	database: Database,
	contributionId: number,
	dataPoints: BitrateDataPoint[],
): Promise<void> {
	const BATCH_SIZE = 500;
	const FIELDS_PER_ROW = 4;

	for (let i = 0; i < dataPoints.length; i += BATCH_SIZE) {
		const batch = dataPoints.slice(i, i + BATCH_SIZE);
		const placeholders: string[] = [];
		const values: unknown[] = [];

		batch.forEach((point, idx) => {
			const offset = idx * FIELDS_PER_ROW;
			placeholders.push(
				`($${offset + 1}, $${offset + 2}, $${offset + 3}, $${offset + 4})`,
			);
			values.push(
				contributionId,
				point.timestamp,
				point.bitrate,
				point.frame_type ?? null,
			);
		});

		// Validate placeholder count matches values count
		const expectedParams = batch.length * FIELDS_PER_ROW;
		if (values.length !== expectedParams) {
			throw new Error(
				`Batch insert validation failed: expected ${expectedParams} values, got ${values.length}`,
			);
		}

		await database.execute(
			`INSERT INTO stream_data_points (contribution_id, timestamp, bitrate, frame_type)
			 VALUES ${placeholders.join(", ")}`,
			values,
		);
	}
}

/**
 * Get bitrate analysis by file path and optional hash validation
 */
export async function getBitrateAnalysis(
	filePath: string,
	currentFileHash?: string,
): Promise<OverallBitrateAnalysis | null> {
	const database = await getDatabase();

	// Get the analysis record
	const analysisRows = await database.select<BitrateAnalysisRow[]>(
		"SELECT * FROM bitrate_analysis WHERE file_path = $1 ORDER BY created_at DESC LIMIT 1",
		[filePath],
	);

	if (analysisRows.length === 0) {
		return null;
	}

	const analysis = analysisRows[0];

	// Validate hash if provided
	if (currentFileHash && analysis.file_hash !== currentFileHash) {
		console.log(
			`[Database] Bitrate analysis invalidated for ${filePath}: file hash changed`,
		);
		await deleteBitrateAnalysisByPath(filePath);
		return null;
	}

	// Get data points
	const dataPointRows = await database.select<DataPointRow[]>(
		"SELECT * FROM bitrate_data_points WHERE analysis_id = $1 ORDER BY timestamp",
		[analysis.id],
	);

	const dataPoints: BitrateDataPoint[] = dataPointRows.map((row) => ({
		timestamp: row.timestamp,
		bitrate: row.bitrate,
		frame_type: row.frame_type ?? undefined,
	}));

	// Get statistics
	const statsRows = await database.select<BitrateStatisticsRow[]>(
		"SELECT * FROM bitrate_statistics WHERE analysis_id = $1",
		[analysis.id],
	);

	if (statsRows.length === 0) {
		console.error(`[Database] Missing statistics for analysis ${analysis.id}`);
		return null;
	}

	const stats = statsRows[0];

	// Get peak intervals
	const peakRows = await database.select<PeakIntervalRow[]>(
		"SELECT * FROM peak_intervals WHERE statistics_id = $1",
		[stats.id],
	);

	const statistics: BitrateStatistics = {
		min_bitrate: stats.min_bitrate,
		max_bitrate: stats.max_bitrate,
		avg_bitrate: stats.avg_bitrate,
		median_bitrate: stats.median_bitrate,
		std_deviation: stats.std_deviation,
		total_frames: stats.total_frames,
		peak_intervals: peakRows.map((row) => ({
			start_time: row.start_time,
			end_time: row.end_time,
			peak_bitrate: row.peak_bitrate,
			duration: row.duration,
		})),
	};

	// Get stream contributions
	const contribRows = await database.select<StreamContributionRow[]>(
		"SELECT * FROM stream_contributions WHERE analysis_id = $1",
		[analysis.id],
	);

	const streamContributions: StreamContribution[] = await Promise.all(
		contribRows.map(async (contrib) => {
			const streamDataRows = await database.select<DataPointRow[]>(
				"SELECT * FROM stream_data_points WHERE contribution_id = $1 ORDER BY timestamp",
				[contrib.id],
			);

			return {
				stream_index: contrib.stream_index,
				stream_type: contrib.stream_type,
				codec_name: contrib.codec_name,
				percentage: contrib.percentage,
				data_points: streamDataRows.map((row) => ({
					timestamp: row.timestamp,
					bitrate: row.bitrate,
					frame_type: row.frame_type ?? undefined,
				})),
			};
		}),
	);

	console.log(
		`[Database] Retrieved bitrate analysis for ${filePath} from cache`,
	);

	return {
		path: analysis.file_path,
		duration: analysis.duration,
		data_points: dataPoints,
		statistics,
		stream_contributions: streamContributions,
		from_cache: true,
	};
}

/**
 * Delete bitrate analysis by file path
 */
export async function deleteBitrateAnalysisByPath(
	filePath: string,
): Promise<number> {
	const database = await getDatabase();

	// Due to ON DELETE CASCADE, this will delete all related records
	const result = await database.execute(
		"DELETE FROM bitrate_analysis WHERE file_path = $1",
		[filePath],
	);

	if (result.rowsAffected > 0) {
		console.log(
			`[Database] Deleted ${result.rowsAffected} bitrate analysis for ${filePath}`,
		);
	}

	return result.rowsAffected;
}

/**
 * Clear all bitrate analysis data
 */
export async function clearAllBitrateAnalysis(): Promise<number> {
	const database = await getDatabase();

	const result = await database.execute("DELETE FROM bitrate_analysis");

	console.log(`[Database] Cleared ${result.rowsAffected} bitrate analyses`);
	return result.rowsAffected;
}

/**
 * Get bitrate analysis statistics
 */
export async function getBitrateAnalysisStats(): Promise<{
	total_analyses: number;
	total_data_points: number;
	oldest_analysis: string | null;
	newest_analysis: string | null;
}> {
	const database = await getDatabase();

	const countRows = await database.select<{ count: number }[]>(
		"SELECT COUNT(*) as count FROM bitrate_analysis",
	);

	const dataPointCountRows = await database.select<{ count: number }[]>(
		"SELECT COUNT(*) as count FROM bitrate_data_points",
	);

	const oldestRows = await database.select<{ created_at: string }[]>(
		"SELECT created_at FROM bitrate_analysis ORDER BY created_at ASC LIMIT 1",
	);

	const newestRows = await database.select<{ created_at: string }[]>(
		"SELECT created_at FROM bitrate_analysis ORDER BY created_at DESC LIMIT 1",
	);

	return {
		total_analyses: countRows[0]?.count ?? 0,
		total_data_points: dataPointCountRows[0]?.count ?? 0,
		oldest_analysis: oldestRows[0]?.created_at ?? null,
		newest_analysis: newestRows[0]?.created_at ?? null,
	};
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Initialize the database (call on app startup)
 */
export async function initDatabase(): Promise<void> {
	await getDatabase();
	console.log("[Database] Initialized successfully");
}

/**
 * Run database maintenance (cleanup old jobs and expired cache)
 */
export async function runMaintenance(): Promise<{
	jobsCleaned: number;
	cacheCleaned: number;
}> {
	const jobsCleaned = await cleanupOldJobs();
	const cacheCleaned = await cleanupExpiredCache();

	console.log(
		`[Database] Maintenance complete: ${jobsCleaned} jobs, ${cacheCleaned} cache entries cleaned`,
	);

	return { jobsCleaned, cacheCleaned };
}
