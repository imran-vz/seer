import { invoke } from "@tauri-apps/api/core";
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
} from "@/types/bitrate";

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

	// Cache stats
	cacheStats: {
		total_analyses: number;
		total_data_points: number;
	} | null;

	// Actions
	analyzeStream: (path: string, streamIndex: number) => Promise<void>;
	analyzeOverall: (path: string, fileHash?: string) => Promise<void>;
	forceAnalyze: (path: string) => Promise<void>;
	cancelAnalysis: () => Promise<void>;
	setAnalysisMode: (mode: "overall" | "per-stream") => void;
	setSelectedStreamIndex: (index: number | null) => void;
	setIntervalSeconds: (interval: number) => void;
	clearAnalysis: () => void;
	clearCache: () => Promise<number>;
	clearCacheForFile: (filePath: string) => Promise<number>;
	exportData: (format: "json" | "csv") => void;
	getJobStatus: () => Promise<JobStatus[]>;
	refreshCacheStats: () => Promise<void>;
}

export const useBitrateStore = create<BitrateState>((set, get) => ({
	currentAnalysis: null,
	loading: false,
	error: null,
	analysisMode: "overall",
	selectedStreamIndex: null,
	intervalSeconds: 1.0,
	currentJobPath: null,
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
			} else {
				set({
					error: errorMessage,
					loading: false,
					currentJobPath: null,
				});
			}
		}
	},

	analyzeOverall: async (path: string, fileHash?: string) => {
		const { currentJobPath, intervalSeconds } = get();

		// If already analyzing this file, don't start another
		if (currentJobPath === path) {
			console.log("[BitrateStore] Analysis already in progress for this file");
			return;
		}

		console.log(`[BitrateStore] Starting overall analysis: ${path}`);
		set({ loading: true, error: null, currentJobPath: path });

		try {
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

			// No cache hit, perform analysis
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
				result.from_cache ? "(from Rust cache)" : "(fresh)",
			);

			// Save to database if it's a fresh analysis
			if (!result.from_cache && fileHash) {
				try {
					// Get file size from the result or use 0 as fallback
					const fileSize = 0; // We'll get this from metadata if needed
					await saveBitrateAnalysis(
						result,
						fileHash,
						fileSize,
						intervalSeconds,
					);
					console.log("[BitrateStore] Saved analysis to database");
					await get().refreshCacheStats();
				} catch (saveError) {
					console.error(
						"[BitrateStore] Failed to save analysis to database:",
						saveError,
					);
					// Don't fail the whole operation if caching fails
				}
			}

			set({ currentAnalysis: result, loading: false, currentJobPath: null });
		} catch (error) {
			console.error("[BitrateStore] Overall analysis error:", error);
			const errorMessage =
				error instanceof Error ? error.message : String(error);

			// Don't show error if it was a cancellation or already running
			if (
				errorMessage.includes("cancelled") ||
				errorMessage.includes("already in progress")
			) {
				set({ loading: false, currentJobPath: null });
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

			// Also clear Rust-side cache
			await invoke("clear_bitrate_cache_cmd");

			// Reset state and analyze
			set({ currentAnalysis: null, error: null });
			await get().analyzeOverall(path);
			await get().refreshCacheStats();
		} catch (error) {
			console.error("[BitrateStore] Force analyze error:", error);
		}
	},

	cancelAnalysis: async () => {
		const { currentJobPath } = get();
		if (!currentJobPath) {
			console.log("[BitrateStore] No analysis to cancel");
			return;
		}

		console.log(`[BitrateStore] Cancelling analysis for: ${currentJobPath}`);
		try {
			const cancelled = await invoke<boolean>("cancel_bitrate_analysis", {
				path: currentJobPath,
			});
			if (cancelled) {
				console.log("[BitrateStore] Analysis cancelled successfully");
				set({ loading: false, currentJobPath: null });
			} else {
				console.log("[BitrateStore] No active job found to cancel");
			}
		} catch (error) {
			console.error("[BitrateStore] Failed to cancel analysis:", error);
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
			// Clear database cache
			const dbCount = await clearAllBitrateAnalysis();

			// Also clear Rust-side file cache
			const rustCount = await invoke<number>("clear_bitrate_cache_cmd");

			const totalCount = dbCount + rustCount;
			console.log(
				`[BitrateStore] Cleared ${dbCount} DB analyses + ${rustCount} file cache = ${totalCount} total`,
			);

			await get().refreshCacheStats();
			return totalCount;
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
}));
