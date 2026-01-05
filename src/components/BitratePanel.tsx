import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useBitrateStore } from "@/stores/bitrateStore";
import type { OverallBitrateAnalysis } from "@/types/bitrate";
import { BitrateChart, type BitrateChartHandle } from "./BitrateChart";
import { BitrateStatisticsComponent } from "./BitrateStatistics";

interface BitrateProgress {
	current: number;
	total: number;
	percentage: number;
	stage: string;
	/** Estimated seconds remaining */
	eta_seconds?: number;
	/** Elapsed seconds since analysis started */
	elapsed_seconds?: number;
	/** Whether sampling mode is being used (for large files) */
	using_sampling?: boolean;
	/** Number of streams being analyzed */
	stream_count?: number;
	/** Current stream being analyzed (1-indexed) */
	current_stream?: number;
}

interface BitratePanelProps {
	filePath: string | null;
}

export function BitratePanel({ filePath }: BitratePanelProps) {
	const {
		currentAnalysis,
		loading,
		error,
		analysisMode,
		selectedStreamIndex,
		currentJobPath,
		queueStatus,
		analyzeOverall,
		forceAnalyze,
		cancelAnalysis,
		clearAnalysis,
		exportData,
		cacheStats,
		refreshCacheStats,
	} = useBitrateStore();

	const chartRef = useRef<BitrateChartHandle>(null);
	const [progress, setProgress] = useState<BitrateProgress | null>(null);
	const [showWarning, setShowWarning] = useState(false);
	const [pendingPath, setPendingPath] = useState<string | null>(null);
	const [lastAnalyzedPath, setLastAnalyzedPath] = useState<string | null>(null);

	// Refresh cache stats on mount
	useEffect(() => {
		refreshCacheStats();
	}, [refreshCacheStats]);

	// Restore progress from queue status when switching files
	// This handles the case where we switch back to a file that's still being analyzed
	useEffect(() => {
		if (!filePath || !queueStatus) return;

		// Find any job for this file path
		const job =
			queueStatus.queued.find(
				(j) => j.path === filePath && j.state.includes("bitrate_analysis"),
			) ||
			queueStatus.running.find(
				(j) => j.path === filePath && j.state.includes("bitrate_analysis"),
			);

		if (job) {
			// There's an active job for this file - restore progress
			console.log("[BitratePanel] Found active job for file:", job);
			if (job.progress_stage) {
				setProgress({
					current: job.progress_current || 0,
					total: job.progress_total || 100,
					percentage: job.progress_percentage || 0,
					stage: job.progress_stage,
				});
			}
			// Mark this file as being analyzed so we don't try to start another job
			setLastAnalyzedPath(filePath);
		} else if (!loading) {
			// Clear progress if no job and not loading
			setProgress(null);
		}
	}, [filePath, queueStatus, loading]);

	useEffect(() => {
		console.log("[BitratePanel] Setting up progress event listener");
		// Listen for progress events
		const unlisten = listen<BitrateProgress>("bitrate-progress", (event) => {
			console.log("[BitratePanel] Progress update:", event.payload);
			setProgress(event.payload);
		});

		return () => {
			console.log("[BitratePanel] Cleaning up progress event listener");
			unlisten.then((fn) => fn());
		};
	}, []);

	useEffect(() => {
		// Check if there's already a running job for this file in the queue
		const hasRunningJob =
			queueStatus?.running.some(
				(j) => j.path === filePath && j.state.includes("bitrate_analysis"),
			) ||
			queueStatus?.queued.some(
				(j) => j.path === filePath && j.state.includes("bitrate_analysis"),
			);

		// Check if we need to analyze a new file
		// Allow starting analysis if:
		// 1. We have a file path
		// 2. It's different from the last analyzed path
		// 3. Either not loading, OR loading a different file (allows switching while bg job runs)
		// 4. There's no already running job for this file
		const isLoadingDifferentFile = loading && currentJobPath !== filePath;
		const canStartAnalysis =
			filePath &&
			filePath !== lastAnalyzedPath &&
			!hasRunningJob &&
			(!loading || isLoadingDifferentFile);

		console.log("[BitratePanel] Analysis check:", {
			filePath,
			lastAnalyzedPath,
			loading,
			currentJobPath,
			isLoadingDifferentFile,
			hasRunningJob,
			canStartAnalysis,
			hasCurrentAnalysis: !!currentAnalysis,
			currentAnalysisPath: currentAnalysis?.path,
		});

		if (canStartAnalysis) {
			// Clear previous analysis if it's for a different file
			if (currentAnalysis && currentAnalysis.path !== filePath) {
				// Don't call clearAnalysis() as it would cancel running job
				// Just reset the progress for the new file
				setProgress(null);
			}

			// Mark this path as being analyzed to prevent re-triggering
			setLastAnalyzedPath(filePath);
			console.log("[BitratePanel] Starting analysis for:", filePath);

			// Start analysis - the store will handle file hash computation
			// We don't use stat() from frontend due to permission issues with arbitrary paths
			analyzeOverall(filePath).catch((err) => {
				console.error("[BitratePanel] Analysis failed:", err);
			});
		} else if (!filePath) {
			clearAnalysis();
			setProgress(null);
			setPendingPath(null);
			setLastAnalyzedPath(null);
		}
	}, [
		filePath,
		lastAnalyzedPath,
		loading,
		currentJobPath,
		currentAnalysis,
		analyzeOverall,
		clearAnalysis,
		queueStatus,
	]);

	const handleConfirmAnalysis = () => {
		if (pendingPath) {
			setLastAnalyzedPath(pendingPath);
			analyzeOverall(pendingPath);
			setShowWarning(false);
			setPendingPath(null);
		}
	};

	const handleCancelAnalysis = () => {
		setShowWarning(false);
		setPendingPath(null);
	};

	const handleExportPng = () => {
		if (chartRef.current) {
			chartRef.current.exportToPng();
		}
	};

	// Check if analysis matches current file
	const analysisMatchesFile =
		currentAnalysis && filePath && currentAnalysis.path === filePath;

	// Check if there's a running/queued job for this file
	const hasActiveJobForFile =
		queueStatus?.running.some(
			(j) => j.path === filePath && j.state.includes("bitrate_analysis"),
		) ||
		queueStatus?.queued.some(
			(j) => j.path === filePath && j.state.includes("bitrate_analysis"),
		);

	// Check if loading is for the current file (not a different file)
	// Also consider if there's an active job in the queue for this file
	const isLoadingForCurrentFile =
		(loading && currentJobPath === filePath) || hasActiveJobForFile;

	// Debug logging for render state
	console.log("[BitratePanel] Render state:", {
		filePath,
		loading,
		currentJobPath,
		isLoadingForCurrentFile,
		analysisMatchesFile,
		error,
		progress,
	});

	// Check if there's a background job running for a different file
	const isLoadingOtherFile =
		loading && currentJobPath && currentJobPath !== filePath;

	// Get filename for display
	const loadingFileName = currentJobPath?.split("/").pop() || currentJobPath;

	// Check if result is from cache
	const isFromCache =
		analysisMatchesFile &&
		"from_cache" in currentAnalysis &&
		(currentAnalysis as OverallBitrateAnalysis).from_cache;

	if (!filePath) {
		return (
			<div className="flex h-full items-center justify-center p-8 text-center text-muted-foreground">
				<div>
					<p className="font-medium text-base">No file selected</p>
					<p className="text-sm">Select a media file to analyze bitrate</p>
					{cacheStats && cacheStats.total_analyses > 0 && (
						<p className="mt-2 text-muted-foreground/60 text-xs">
							{cacheStats.total_analyses} cached analyses (
							{cacheStats.total_data_points.toLocaleString()} data points)
						</p>
					)}
				</div>
			</div>
		);
	}

	// Helper to format seconds as "Xm Ys" or "Xs"
	const formatTime = (seconds: number): string => {
		if (seconds < 60) {
			return `${Math.round(seconds)}s`;
		}
		const mins = Math.floor(seconds / 60);
		const secs = Math.round(seconds % 60);
		return `${mins}m ${secs}s`;
	};

	if (isLoadingForCurrentFile) {
		return (
			<div className="flex h-full items-center justify-center">
				<div className="w-full max-w-md px-8 text-center">
					<div className="mb-4 font-medium text-muted-foreground text-sm">
						{progress?.using_sampling
							? "Analyzing bitrate (sampling mode)..."
							: "Analyzing bitrate..."}
					</div>
					{progress && (
						<>
							{/* Progress bar */}
							<div className="mb-2 h-2 w-full overflow-hidden rounded-full bg-muted">
								<div
									className="h-full bg-primary transition-all duration-300"
									style={{ width: `${progress.percentage}%` }}
								/>
							</div>
							{/* Progress text */}
							<div className="flex items-center justify-between text-xs">
								<span className="text-muted-foreground">{progress.stage}</span>
								<span className="font-medium text-muted-foreground">
									{Math.round(progress.percentage)}%
								</span>
							</div>
							{/* ETA and elapsed time */}
							<div className="mt-2 flex items-center justify-between text-muted-foreground/70 text-xs">
								<span>
									{progress.elapsed_seconds !== undefined
										? `Elapsed: ${formatTime(progress.elapsed_seconds)}`
										: ""}
								</span>
								<span>
									{progress.eta_seconds !== undefined &&
									progress.eta_seconds > 0
										? `ETA: ${formatTime(progress.eta_seconds)}`
										: ""}
								</span>
							</div>
							{/* Stream count info */}
							{progress.stream_count !== undefined &&
								progress.stream_count > 1 && (
									<div className="mt-1 text-muted-foreground/60 text-xs">
										{progress.current_stream !== undefined
											? `Stream ${progress.current_stream}/${progress.stream_count}`
											: `${progress.stream_count} streams`}
									</div>
								)}
							{/* Sampling mode indicator */}
							{progress.using_sampling && (
								<div className="mt-1 text-amber-500/80 text-xs">
									Large file detected — using sampling for faster analysis
								</div>
							)}
						</>
					)}
					{!progress && (
						<div className="text-muted-foreground/60 text-xs">
							Initializing analysis...
						</div>
					)}
					{/* Cancel button */}
					<Button
						type="button"
						variant="outline"
						size="sm"
						className="mt-4"
						onClick={() => cancelAnalysis()}
					>
						Cancel
					</Button>
				</div>
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex h-full items-center justify-center p-8">
				<div className="text-center">
					<p className="mb-2 font-medium text-destructive text-sm">
						Error analyzing bitrate
					</p>
					<p className="text-muted-foreground text-xs">{error}</p>
					<Button
						type="button"
						variant="outline"
						size="sm"
						className="mt-4"
						onClick={() => {
							clearAnalysis();
							setLastAnalyzedPath(null);
						}}
					>
						Retry
					</Button>
				</div>
			</div>
		);
	}

	if (!currentAnalysis || !analysisMatchesFile) {
		return (
			<div className="flex h-full items-center justify-center p-8 text-center text-muted-foreground">
				<div>
					<p className="font-medium text-base">No analysis data</p>
					<p className="text-sm">
						{isLoadingOtherFile
							? "Another analysis is running in the background"
							: "Select this file's Bitrate tab to analyze"}
					</p>
					{isLoadingOtherFile && loadingFileName && (
						<p className="mt-2 max-w-xs truncate text-muted-foreground/60 text-xs">
							Analyzing: {loadingFileName}
						</p>
					)}
				</div>
			</div>
		);
	}

	return (
		<>
			<ScrollArea className="h-full">
				<div className="space-y-4 p-4">
					{/* Header with export buttons */}
					<div className="flex flex-col items-start justify-between gap-4">
						<div>
							<div className="flex flex-nowrap items-center gap-2">
								<h3 className="text-nowrap font-semibold text-base">
									Bitrate Analysis
								</h3>
								{isFromCache && (
									<span className="rounded-full bg-muted px-2 py-0.5 text-muted-foreground text-xs">
										Cached
									</span>
								)}
							</div>
							<p className="text-muted-foreground text-xs">
								{analysisMode === "overall"
									? "Overall file bitrate"
									: `Stream ${selectedStreamIndex} bitrate`}
							</p>
						</div>
						<div className="flex flex-1 flex-wrap gap-2">
							{isFromCache && (
								<Button
									type="button"
									variant="outline"
									size="sm"
									onClick={() => forceAnalyze(filePath)}
								>
									Re-analyze
								</Button>
							)}
							<Button
								type="button"
								variant="outline"
								size="sm"
								onClick={handleExportPng}
							>
								Export PNG
							</Button>
							<Button
								type="button"
								variant="outline"
								size="sm"
								onClick={() => exportData("json")}
							>
								Export JSON
							</Button>
							<Button
								type="button"
								variant="outline"
								size="sm"
								onClick={() => exportData("csv")}
							>
								Export CSV
							</Button>
						</div>
					</div>

					{/* Bitrate Chart */}
					<BitrateChart
						ref={chartRef}
						dataPoints={currentAnalysis.data_points}
						duration={currentAnalysis.duration}
						streamContributions={
							"stream_contributions" in currentAnalysis
								? (currentAnalysis as OverallBitrateAnalysis)
										.stream_contributions
								: undefined
						}
					/>

					{/* Statistics */}
					<BitrateStatisticsComponent statistics={currentAnalysis.statistics} />
				</div>
			</ScrollArea>

			{/* Large file warning dialog */}
			<AlertDialog open={showWarning} onOpenChange={setShowWarning}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Large File Detected</AlertDialogTitle>
						<AlertDialogDescription>
							This file is larger than 1 GB. Bitrate analysis may take several
							minutes and could temporarily freeze the UI. Do you want to
							continue?
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel onClick={handleCancelAnalysis}>
							Cancel
						</AlertDialogCancel>
						<AlertDialogAction onClick={handleConfirmAnalysis}>
							Continue Analysis
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	);
}
