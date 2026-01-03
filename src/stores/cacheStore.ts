/**
 * Cache store for managing cached data
 * Integrates with SQLite database for persistence
 */

import { create } from "zustand";
import {
	cleanupExpiredCache,
	clearAllCache,
	deleteCacheByFilePath,
	deleteCacheByType,
	generateCacheKey,
	getCacheStatistics,
	getValidCache,
	setCache,
} from "@/lib/database";
import type {
	CacheStatistics,
	CacheType,
	CacheWithParsedData,
	CreateCacheParams,
} from "@/types/database";

interface CacheState {
	// State
	statistics: CacheStatistics | null;
	loading: boolean;
	error: string | null;

	// Actions
	refreshStats: () => Promise<void>;

	// Cache operations
	get: (
		cacheType: CacheType,
		filePath: string,
		params?: Record<string, unknown>,
		currentFileHash?: string,
	) => Promise<CacheWithParsedData | null>;

	set: (params: CreateCacheParams) => Promise<void>;

	// Invalidation
	invalidateFile: (filePath: string) => Promise<number>;
	invalidateType: (cacheType: CacheType) => Promise<number>;
	clearAll: () => Promise<number>;

	// Maintenance
	cleanup: () => Promise<number>;
}

export const useCacheStore = create<CacheState>((set, get) => ({
	statistics: null,
	loading: false,
	error: null,

	refreshStats: async () => {
		set({ loading: true, error: null });
		try {
			const stats = await getCacheStatistics();
			set({ statistics: stats, loading: false });
		} catch (error) {
			console.error("[CacheStore] Failed to refresh stats:", error);
			set({
				error: error instanceof Error ? error.message : String(error),
				loading: false,
			});
		}
	},

	get: async (
		cacheType: CacheType,
		filePath: string,
		params?: Record<string, unknown>,
		currentFileHash?: string,
	): Promise<CacheWithParsedData | null> => {
		try {
			const cacheKey = generateCacheKey(cacheType, filePath, params);
			const cached = await getValidCache(cacheKey, currentFileHash);

			if (cached) {
				console.log(`[CacheStore] Cache hit: ${cacheType} for ${filePath}`);
				return cached;
			}

			console.log(`[CacheStore] Cache miss: ${cacheType} for ${filePath}`);
			return null;
		} catch (error) {
			console.error("[CacheStore] Failed to get cache:", error);
			return null;
		}
	},

	set: async (params: CreateCacheParams) => {
		try {
			await setCache(params);
			console.log(
				`[CacheStore] Cached ${params.cache_type} for ${params.file_path}`,
			);

			// Refresh stats
			await get().refreshStats();
		} catch (error) {
			console.error("[CacheStore] Failed to set cache:", error);
			throw error;
		}
	},

	invalidateFile: async (filePath: string) => {
		try {
			const count = await deleteCacheByFilePath(filePath);
			console.log(
				`[CacheStore] Invalidated ${count} cache entries for ${filePath}`,
			);

			// Refresh stats
			await get().refreshStats();

			return count;
		} catch (error) {
			console.error("[CacheStore] Failed to invalidate file cache:", error);
			throw error;
		}
	},

	invalidateType: async (cacheType: CacheType) => {
		try {
			const count = await deleteCacheByType(cacheType);
			console.log(
				`[CacheStore] Invalidated ${count} cache entries of type ${cacheType}`,
			);

			// Refresh stats
			await get().refreshStats();

			return count;
		} catch (error) {
			console.error("[CacheStore] Failed to invalidate type cache:", error);
			throw error;
		}
	},

	clearAll: async () => {
		try {
			const count = await clearAllCache();
			console.log(`[CacheStore] Cleared ${count} cache entries`);

			// Refresh stats
			await get().refreshStats();

			return count;
		} catch (error) {
			console.error("[CacheStore] Failed to clear cache:", error);
			throw error;
		}
	},

	cleanup: async () => {
		try {
			const count = await cleanupExpiredCache();
			console.log(`[CacheStore] Cleaned up ${count} expired cache entries`);

			// Refresh stats
			await get().refreshStats();

			return count;
		} catch (error) {
			console.error("[CacheStore] Failed to cleanup cache:", error);
			throw error;
		}
	},
}));
