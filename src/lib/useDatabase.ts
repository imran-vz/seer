/**
 * React hooks for database operations
 * Provides easy access to job and cache operations in components
 */

import { useCallback, useEffect, useState } from "react";
import type {
	CacheStatistics,
	CacheType,
	CacheWithParsedData,
	CreateCacheParams,
	CreateJobParams,
	Job,
	JobStatistics,
	UpdateJobParams,
} from "@/types/database";
import {
	cancelJob,
	cleanupExpiredCache,
	clearAllCache,
	completeJob,
	createJob,
	deleteCacheByFilePath,
	failJob,
	generateCacheKey,
	getCacheStatistics,
	getJobStatistics,
	getRecentJobs,
	getRunningJobs,
	getValidCache,
	initDatabase,
	runMaintenance,
	setCache,
	startJob,
	updateJob,
} from "./database";

// ============================================================================
// Database Initialization Hook
// ============================================================================

/**
 * Hook to initialize the database on app startup
 */
export function useInitDatabase(): {
	initialized: boolean;
	error: string | null;
} {
	const [initialized, setInitialized] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		initDatabase()
			.then(() => {
				setInitialized(true);
				// Run maintenance on startup
				runMaintenance().catch(console.error);
			})
			.catch((err) => {
				console.error("[useInitDatabase] Failed to initialize:", err);
				setError(err instanceof Error ? err.message : String(err));
			});
	}, []);

	return { initialized, error };
}

// ============================================================================
// Job Hooks
// ============================================================================

/**
 * Hook for managing jobs
 */
export function useJobs(autoRefresh = false, refreshInterval = 5000) {
	const [jobs, setJobs] = useState<Job[]>([]);
	const [runningJobs, setRunningJobs] = useState<Job[]>([]);
	const [statistics, setStatistics] = useState<JobStatistics | null>(null);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);

	const refresh = useCallback(async () => {
		try {
			const [recentJobs, running, stats] = await Promise.all([
				getRecentJobs(50),
				getRunningJobs(),
				getJobStatistics(),
			]);
			setJobs(recentJobs);
			setRunningJobs(running);
			setStatistics(stats);
			setError(null);
		} catch (err) {
			console.error("[useJobs] Failed to fetch jobs:", err);
			setError(err instanceof Error ? err.message : String(err));
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		refresh();

		if (autoRefresh) {
			const interval = setInterval(refresh, refreshInterval);
			return () => clearInterval(interval);
		}
	}, [autoRefresh, refreshInterval, refresh]);

	const create = useCallback(
		async (params: CreateJobParams): Promise<Job> => {
			const job = await createJob(params);
			await refresh();
			return job;
		},
		[refresh],
	);

	const update = useCallback(
		async (id: number, updates: UpdateJobParams): Promise<Job | null> => {
			const job = await updateJob(id, updates);
			await refresh();
			return job;
		},
		[refresh],
	);

	const start = useCallback(
		async (id: number): Promise<Job | null> => {
			const job = await startJob(id);
			await refresh();
			return job;
		},
		[refresh],
	);

	const complete = useCallback(
		async (
			id: number,
			result?: Record<string, unknown>,
		): Promise<Job | null> => {
			const job = await completeJob(id, result);
			await refresh();
			return job;
		},
		[refresh],
	);

	const fail = useCallback(
		async (id: number, errorMessage: string): Promise<Job | null> => {
			const job = await failJob(id, errorMessage);
			await refresh();
			return job;
		},
		[refresh],
	);

	const cancel = useCallback(
		async (id: number): Promise<Job | null> => {
			const job = await cancelJob(id);
			await refresh();
			return job;
		},
		[refresh],
	);

	return {
		jobs,
		runningJobs,
		statistics,
		loading,
		error,
		refresh,
		create,
		update,
		start,
		complete,
		fail,
		cancel,
	};
}

// ============================================================================
// Cache Hooks
// ============================================================================

/**
 * Hook for managing cache
 */
export function useCache() {
	const [statistics, setStatistics] = useState<CacheStatistics | null>(null);
	const [loading, setLoading] = useState(true);
	const [error, setError] = useState<string | null>(null);

	const refreshStats = useCallback(async () => {
		try {
			const stats = await getCacheStatistics();
			setStatistics(stats);
			setError(null);
		} catch (err) {
			console.error("[useCache] Failed to fetch statistics:", err);
			setError(err instanceof Error ? err.message : String(err));
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		refreshStats();
	}, [refreshStats]);

	const get = useCallback(
		async (
			cacheType: CacheType,
			filePath: string,
			params?: Record<string, unknown>,
			currentFileHash?: string,
		): Promise<CacheWithParsedData | null> => {
			const cacheKey = generateCacheKey(cacheType, filePath, params);
			return getValidCache(cacheKey, currentFileHash);
		},
		[],
	);

	const set = useCallback(
		async (params: CreateCacheParams): Promise<void> => {
			await setCache(params);
			await refreshStats();
		},
		[refreshStats],
	);

	const invalidateFile = useCallback(
		async (filePath: string): Promise<number> => {
			const count = await deleteCacheByFilePath(filePath);
			await refreshStats();
			return count;
		},
		[refreshStats],
	);

	const clearAll = useCallback(async (): Promise<number> => {
		const count = await clearAllCache();
		await refreshStats();
		return count;
	}, [refreshStats]);

	const cleanup = useCallback(async (): Promise<number> => {
		const count = await cleanupExpiredCache();
		await refreshStats();
		return count;
	}, [refreshStats]);

	return {
		statistics,
		loading,
		error,
		refreshStats,
		get,
		set,
		invalidateFile,
		clearAll,
		cleanup,
	};
}

/**
 * Hook for caching a specific piece of data
 * Automatically handles cache lookup and storage
 */
export function useCachedData<T>(
	cacheType: CacheType,
	filePath: string | null,
	params?: Record<string, unknown>,
) {
	const [data, setData] = useState<T | null>(null);
	const [loading, setLoading] = useState(false);
	const [fromCache, setFromCache] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const cacheKey = filePath
		? generateCacheKey(cacheType, filePath, params)
		: null;

	const loadFromCache = useCallback(
		async (currentFileHash?: string): Promise<T | null> => {
			if (!cacheKey) return null;

			try {
				setLoading(true);
				const cached = await getValidCache(cacheKey, currentFileHash);
				if (cached) {
					setData(cached.data as T);
					setFromCache(true);
					return cached.data as T;
				}
				return null;
			} catch (err) {
				console.error("[useCachedData] Failed to load from cache:", err);
				setError(err instanceof Error ? err.message : String(err));
				return null;
			} finally {
				setLoading(false);
			}
		},
		[cacheKey],
	);

	const saveToCache = useCallback(
		async (
			newData: T,
			fileInfo: {
				fileHash: string;
				fileSize: number;
				fileModifiedAt: string;
			},
			ttlSeconds?: number,
		): Promise<void> => {
			if (!cacheKey || !filePath) return;

			try {
				await setCache({
					cache_type: cacheType,
					cache_key: cacheKey,
					file_path: filePath,
					file_hash: fileInfo.fileHash,
					file_size: fileInfo.fileSize,
					file_modified_at: fileInfo.fileModifiedAt,
					data: newData as Record<string, unknown>,
					ttl_seconds: ttlSeconds,
				});
				setData(newData);
				setFromCache(false);
			} catch (err) {
				console.error("[useCachedData] Failed to save to cache:", err);
				setError(err instanceof Error ? err.message : String(err));
			}
		},
		[cacheKey, cacheType, filePath],
	);

	const invalidate = useCallback(async (): Promise<void> => {
		if (!filePath) return;
		await deleteCacheByFilePath(filePath);
		setData(null);
		setFromCache(false);
	}, [filePath]);

	return {
		data,
		loading,
		fromCache,
		error,
		loadFromCache,
		saveToCache,
		invalidate,
		setData,
	};
}
