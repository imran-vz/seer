import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { stat } from "@tauri-apps/plugin-fs";
import { toast } from "sonner";
import { create } from "zustand";
import {
	clearAllBitrateAnalysis,
	deleteBitrateAnalysisByPath,
	getBitrateAnalysis,
	getBitrateAnalysisStats,
	saveBitrateAnalysis,
} from "@/lib/database";
import type {
	BitrateAnalysis,
	JobStatus,
	OverallBitrateAnalysis,
	QueueStatus,
} from "@/types/bitrate";

/**
 * Compute file hash using the backend command
 * This uses file size, mtime, and sample bytes for fast cache validation
 */
async function computeFileHash(path: string): Promise<string> {
	try {
		return await invoke<string>("compute_file_hash_cmd", { path });
	} catch (error) {
		console.error("[BitrateStore] Failed to compute file hash:", error);
		// Return a timestamp-based fallback that won't match any cached hash
		return `fallback-${Date.now()}`;
	}
}

/**
 * Get file size using Tauri fs plugin
 */
async function getFileSize(path: string): Promise<number> {
	try {
		const fileStat = await stat(path);
		return fileStat.size;
	} catch {
		return 0;
	}
}

interface BitrateState {
	currentAnalysis: BitrateAnalysis | OverallBitrateAnalysis | null;
	loading: boolean;
	error: string | null;

	// Analysis mode
	analysisMode: "overall" | "per-stream";
	selectedStreamIndex: number | null;
	intervalSeconds: number;

	// Job tracking
	currentJobPath: string | null;
	queueStatus: QueueStatus | null;

	// Cache stats
	cacheStats: {
		total_analyses: number;
		total_data_points: number;
	} | null;

	// Actions
	analyzeStream: (path: string, streamIndex: number) => Promise<void>;
	analyzeOverall: (path: string, fileHash?: string) => Promise<void>;
	forceAnalyze: (path: string) => Promise<void>;
	cancelAnalysis: (path?: string) => Promise<void>;
	cancelAllJobs: () => Promise<void>;
	setAnalysisMode: (mode: "overall" | "per-stream") => void;
	setSelectedStreamIndex: (index: number | null) => void;
	setIntervalSeconds: (interval: number) => void;
	clearAnalysis: () => void;
	clearCache: () => Promise<number>;
	clearCacheForFile: (filePath: string) => Promise<number>;
	exportData: (format: "json" | "csv") => void;
	getJobStatus: () => Promise<JobStatus[]>;
	refreshCacheStats: () => Promise<void>;
	computeFileHash: (path: string) => Promise<string>;
}

// Set up event listener for queue updates
listen<QueueStatus>("job-queue-update", (event) => {
	useBitrateStore.setState({ queueStatus: event.payload });
}).catch((error) => {
	console.error("[BitrateStore] Failed to set up queue listener:", error);
});

export const useBitrateStore = create<BitrateState>((set, get) => ({
	currentAnalysis: null,
	loading: false,
	error: null,
	analysisMode: "overall",
	selectedStreamIndex: null,
	intervalSeconds: 1.0,
	currentJobPath: null,
	queueStatus: null,
	cacheStats: null,

	analyzeStream: async (path: string, streamIndex: number) => {
		const { currentJobPath } = get();

		// If already analyzing this file, don't start another
		if (currentJobPath === path) {
			console.log("[BitrateStore] Analysis already in progress for this file");
			return;
		}

		console.log(
			`[BitrateStore] Starting stream analysis: ${path}, stream ${streamIndex}`,
		);
		set({ loading: true, error: null, currentJobPath: path });

		try {
			const result = await invoke<BitrateAnalysis>("analyze_stream_bitrate", {
				path,
				streamIndex,
				intervalSeconds: get().intervalSeconds,
			});
			console.log(
				"[BitrateStore] Stream analysis complete:",
				result.statistics,
			);
			set({ currentAnalysis: result, loading: false, currentJobPath: null });
		} catch (error) {
			console.error("[BitrateStore] Stream analysis error:", error);
			const errorMessage =
				error instanceof Error ? error.message : String(error);

			// Don't show error if it was a cancellation
			if (errorMessage.includes("cancelled")) {
				set({ loading: false, currentJobPath: null });
			}
			// If already queued/running, keep loading state to show progress
			else if (
				errorMessage.includes("already queued") ||
				errorMessage.includes("already in progress")
			) {
				console.log("[BitrateStore] Job already queued/running, showing existing progress");
				// Keep loading: true and currentJobPath set so progress events are displayed
			} else {
				set({
					error: errorMessage,
					loading: false,
					currentJobPath: null,
				});
			}
		}
	},

	analyzeOverall: async (path: string, providedFileHash?: string) => {
		const { currentJobPath, intervalSeconds } = get();

		// If already analyzing this file, don't start another
		if (currentJobPath === path) {
			console.log("[BitrateStore] Analysis already in progress for this file");
			return;
		}

		console.log(`[BitrateStore] Starting overall analysis: ${path}`);
		set({ loading: true, error: null, currentJobPath: path });

		try {
			// Compute file hash using backend if not provided
			const fileHash = providedFileHash || (await computeFileHash(path));

			// Check database cache first
			const cached = await getBitrateAnalysis(path, fileHash);
			if (cached) {
				console.log("[BitrateStore] Returning cached analysis from database");
				set({
					currentAnalysis: cached,
					loading: false,
					currentJobPath: null,
				});
				return;
			}

			// No cache hit, perform analysis via backend
			const result = await invoke<OverallBitrateAnalysis>(
				"analyze_overall_bitrate",
				{
					path,
					intervalSeconds,
				},
			);

			console.log(
				"[BitrateStore] Overall analysis complete:",
				result.statistics,
			);

			// Save to database cache
			try {
				const fileSize = await getFileSize(path);
				await saveBitrateAnalysis(result, fileHash, fileSize, intervalSeconds);
				console.log("[BitrateStore] Saved analysis to database cache");
				await get().refreshCacheStats();
			} catch (saveError) {
				console.error(
					"[BitrateStore] Failed to save analysis to database:",
					saveError,
				);
				// Don't fail the whole operation if caching fails
			}

			// Mark as from_cache: false since it's a fresh analysis
			set({
				currentAnalysis: { ...result, from_cache: false },
				loading: false,
				currentJobPath: null,
			});
		} catch (error) {
			console.error("[BitrateStore] Overall analysis error:", error);
			const errorMessage =
				error instanceof Error ? error.message : String(error);

			// Don't show error if it was a cancellation
			if (errorMessage.includes("cancelled")) {
				set({ loading: false, currentJobPath: null });
			}
			// If already queued/running, keep loading state to show progress
			else if (
				errorMessage.includes("already queued") ||
				errorMessage.includes("already in progress")
			) {
				console.log("[BitrateStore] Job already queued/running, showing existing progress");
				// Keep loading: true and currentJobPath set so progress events are displayed
				// Don't set error - this is expected behavior
			} else {
				set({
					error: errorMessage,
					loading: false,
					currentJobPath: null,
				});
			}
		}
	},

	forceAnalyze: async (path: string) => {
		console.log(`[BitrateStore] Force analyzing (bypassing cache): ${path}`);

		try {
			// Clear database cache for this file
			await deleteBitrateAnalysisByPath(path);

			// Reset state and analyze
			set({ currentAnalysis: null, error: null });

			// Compute fresh hash
			const fileHash = await computeFileHash(path);

			// Start fresh analysis (will skip cache check since we just cleared it)
			await get().analyzeOverall(path, fileHash);
			await get().refreshCacheStats();
		} catch (error) {
			console.error("[BitrateStore] Force analyze error:", error);
		}
	},

	cancelAnalysis: async (path?: string) => {
		const { currentJobPath } = get();
		const targetPath = path || currentJobPath;

		if (!targetPath) {
			console.log("[BitrateStore] No analysis to cancel");
			return;
		}

		console.log(`[BitrateStore] Cancelling analysis for: ${targetPath}`);
		try {
			const cancelled = await invoke<boolean>("cancel_bitrate_analysis", {
				path: targetPath,
			});
			if (cancelled) {
				console.log("[BitrateStore] Analysis cancelled successfully");
				// Only clear currentJobPath if we cancelled the current job
				if (targetPath === currentJobPath) {
					set({ loading: false, currentJobPath: null });
				}
			} else {
				console.log("[BitrateStore] No active job found to cancel");
			}
		} catch (error) {
			console.error("[BitrateStore] Failed to cancel analysis:", error);
		}
	},

	cancelAllJobs: async () => {
		console.log("[BitrateStore] Cancelling all jobs");
		try {
			await invoke("cancel_all_bitrate_jobs");
			console.log("[BitrateStore] All jobs cancelled successfully");
			set({ loading: false, currentJobPath: null });
		} catch (error) {
			console.error("[BitrateStore] Failed to cancel all jobs:", error);
		}
	},

	setAnalysisMode: (mode) => set({ analysisMode: mode }),

	setSelectedStreamIndex: (index) => set({ selectedStreamIndex: index }),

	setIntervalSeconds: (interval) => set({ intervalSeconds: interval }),

	clearAnalysis: () =>
		set({
			currentAnalysis: null,
			error: null,
			loading: false,
			currentJobPath: null,
		}),

	clearCache: async () => {
		try {
			// Clear database cache only (file-based cache has been removed)
			const dbCount = await clearAllBitrateAnalysis();
			console.log(
				`[BitrateStore] Cleared ${dbCount} cached analyses from database`,
			);
			await get().refreshCacheStats();
			return dbCount;
		} catch (error) {
			console.error("[BitrateStore] Failed to clear cache:", error);
			return 0;
		}
	},

	clearCacheForFile: async (filePath: string) => {
		try {
			const count = await deleteBitrateAnalysisByPath(filePath);
			console.log(
				`[BitrateStore] Cleared ${count} cached analyses for ${filePath}`,
			);
			await get().refreshCacheStats();
			return count;
		} catch (error) {
			console.error("[BitrateStore] Failed to clear cache for file:", error);
			return 0;
		}
	},

	exportData: (format) => {
		const { currentAnalysis } = get();
		if (!currentAnalysis) return;

		let content: string;
		let filename: string;

		if (format === "json") {
			content = JSON.stringify(currentAnalysis, null, 2);
			filename = "bitrate-analysis.json";
		} else {
			// CSV export
			const lines = ["timestamp,bitrate"];
			for (const point of currentAnalysis.data_points) {
				lines.push(`${point.timestamp},${point.bitrate}`);
			}
			content = lines.join("\n");
			filename = "bitrate-analysis.csv";
		}

		// Trigger download
		const blob = new Blob([content], { type: "text/plain" });
		const url = URL.createObjectURL(blob);
		const a = document.createElement("a");
		a.href = url;
		a.download = filename;
		a.click();
		toast.success(`Exported ${filename}`);
		URL.revokeObjectURL(url);
	},

	getJobStatus: async () => {
		try {
			return await invoke<JobStatus[]>("get_bitrate_job_status");
		} catch (error) {
			console.error("[BitrateStore] Failed to get job status:", error);
			return [];
		}
	},

	refreshCacheStats: async () => {
		try {
			const stats = await getBitrateAnalysisStats();
			set({
				cacheStats: {
					total_analyses: stats.total_analyses,
					total_data_points: stats.total_data_points,
				},
			});
		} catch (error) {
			console.error("[BitrateStore] Failed to refresh cache stats:", error);
		}
	},

	computeFileHash: async (path: string) => {
		return computeFileHash(path);
	},
}));
