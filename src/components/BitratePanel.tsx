import { listen } from "@tauri-apps/api/event";
import { stat } from "@tauri-apps/plugin-fs";
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
}

interface BitratePanelProps {
	filePath: string | null;
}

/**
 * Generate a simple hash from file metadata for cache invalidation
 * This is faster than reading file contents
 */
async function computeFileHash(filePath: string): Promise<string> {
	try {
		const fileStat = await stat(filePath);
		// Combine path, size, and mtime for a quick hash
		const hashInput = `${filePath}:${fileStat.size}:${fileStat.mtime?.getTime() ?? 0}`;
		// Simple hash using built-in crypto
		const encoder = new TextEncoder();
		const data = encoder.encode(hashInput);
		const hashBuffer = await crypto.subtle.digest("SHA-256", data);
		const hashArray = Array.from(new Uint8Array(hashBuffer));
		return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
	} catch (error) {
		console.error("[BitratePanel] Failed to compute file hash:", error);
		// Return a timestamp-based fallback that won't match any cached hash
		return `fallback-${Date.now()}`;
	}
}

export function BitratePanel({ filePath }: BitratePanelProps) {
	const {
		currentAnalysis,
		loading,
		error,
		analysisMode,
		selectedStreamIndex,
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
	const [pendingHash, setPendingHash] = useState<string | null>(null);
	const [lastAnalyzedPath, setLastAnalyzedPath] = useState<string | null>(null);

	// Refresh cache stats on mount
	useEffect(() => {
		refreshCacheStats();
	}, [refreshCacheStats]);

	useEffect(() => {
		// Listen for progress events
		const unlisten = listen<BitrateProgress>("bitrate-progress", (event) => {
			console.log("[BitratePanel] Progress update:", event.payload);
			setProgress(event.payload);
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	useEffect(() => {
		// Check if we need to analyze a new file
		if (filePath && filePath !== lastAnalyzedPath && !loading) {
			// Clear previous analysis if it's for a different file
			if (currentAnalysis && currentAnalysis.path !== filePath) {
				clearAnalysis();
				setProgress(null);
			}

			// Get file stats and compute hash
			stat(filePath)
				.then(async (fileStat) => {
					const fileHash = await computeFileHash(filePath);
					const sizeGB = fileStat.size / (1024 * 1024 * 1024);

					if (sizeGB > 1) {
						// Show warning for files > 1GB
						setPendingPath(filePath);
						setPendingHash(fileHash);
						setShowWarning(true);
					} else {
						setLastAnalyzedPath(filePath);
						analyzeOverall(filePath, fileHash);
					}
				})
				.catch(() => {
					// If we can't get file stats, just analyze without hash
					setLastAnalyzedPath(filePath);
					analyzeOverall(filePath);
				});
		} else if (!filePath) {
			clearAnalysis();
			setProgress(null);
			setPendingPath(null);
			setPendingHash(null);
			setLastAnalyzedPath(null);
		}
	}, [
		filePath,
		lastAnalyzedPath,
		loading,
		currentAnalysis,
		analyzeOverall,
		clearAnalysis,
	]);

	const handleConfirmAnalysis = () => {
		if (pendingPath) {
			setLastAnalyzedPath(pendingPath);
			analyzeOverall(pendingPath, pendingHash ?? undefined);
			setShowWarning(false);
			setPendingPath(null);
			setPendingHash(null);
		}
	};

	const handleCancelAnalysis = () => {
		setShowWarning(false);
		setPendingPath(null);
		setPendingHash(null);
	};

	const handleExportPng = () => {
		if (chartRef.current) {
			chartRef.current.exportToPng();
		}
	};

	// Check if analysis matches current file
	const analysisMatchesFile =
		currentAnalysis && filePath && currentAnalysis.path === filePath;

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

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center">
				<div className="w-full max-w-md px-8 text-center">
					<div className="mb-4 font-medium text-muted-foreground text-sm">
						Analyzing bitrate...
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
			<div className="flex h-full items-center justify-center">
				<p className="text-muted-foreground text-sm">
					No analysis data available
				</p>
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
