import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Check, Clock, Loader2, Trash2 } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import {
	Accordion,
	AccordionContent,
	AccordionItem,
	AccordionTrigger,
} from "@/components/ui/accordion";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { QueueStatus } from "@/types/bitrate";

type StreamType =
	| "video"
	| "audio"
	| "subtitle"
	| "attachment"
	| "data"
	| "unknown";

interface StreamInfo {
	index: number;
	stream_type: StreamType;
	codec_name: string | null;
	codec_long_name: string | null;
	language: string | null;
	title: string | null;
	is_default: boolean;
	is_forced: boolean;
	is_hearing_impaired: boolean;
	is_visual_impaired: boolean;
	is_commentary: boolean;
	is_lyrics: boolean;
	is_karaoke: boolean;
	is_cover_art: boolean;
	width: number | null;
	height: number | null;
	frame_rate: string | null;
	pixel_format: string | null;
	sample_rate: string | null;
	channels: number | null;
	channel_layout: string | null;
	bit_rate: string | null;
	subtitle_format: string | null;
	estimated_size: number | null;
}

interface MediaStreams {
	path: string;
	streams: StreamInfo[];
	video_count: number;
	audio_count: number;
	subtitle_count: number;
	attachment_count: number;
	total_size: number;
	duration: number;
}

interface StreamRemovalOp {
	path: string;
	stream_indices: number[];
}

interface BulkStreamRemovalResult {
	jobs_queued: number;
	job_ids: string[];
	errors: string[];
}

interface FileStreamsData {
	path: string;
	streams: MediaStreams | null;
	loading: boolean;
	error: string | null;
	selectedIndices: Set<number>;
}

type JobDisplayState = "queued" | "running" | "completed";

interface JobDisplayInfo {
	path: string;
	state: JobDisplayState;
	progress: number;
	fileName: string;
}

interface BulkStreamCleanupDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	paths: string[];
	onSuccess: () => void;
}

const LANGUAGE_NAMES: Record<string, string> = {
	eng: "English",
	spa: "Spanish",
	fra: "French",
	deu: "German",
	ita: "Italian",
	por: "Portuguese",
	rus: "Russian",
	jpn: "Japanese",
	kor: "Korean",
	chi: "Chinese",
	zho: "Chinese",
	ara: "Arabic",
	hin: "Hindi",
	und: "Undefined",
};

function getLanguageName(code: string | null): string {
	if (!code) return "Unknown";
	return LANGUAGE_NAMES[code.toLowerCase()] || code.toUpperCase();
}

function getStreamTypeIcon(type: StreamType, isCoverArt?: boolean): string {
	if (isCoverArt) return "🖼️";
	switch (type) {
		case "video":
			return "🎬";
		case "audio":
			return "🔊";
		case "subtitle":
			return "💬";
		case "attachment":
			return "📎";
		default:
			return "❓";
	}
}

function formatStreamLabel(stream: StreamInfo): string {
	const type =
		stream.stream_type.charAt(0).toUpperCase() + stream.stream_type.slice(1);
	const parts = [
		`${getStreamTypeIcon(stream.stream_type, stream.is_cover_art)} ${type}`,
	];

	if (stream.codec_name) {
		parts.push(stream.codec_name.toUpperCase());
	}

	if (stream.language) {
		parts.push(getLanguageName(stream.language));
	}

	if (stream.title) {
		parts.push(`"${stream.title}"`);
	}

	// Add flags
	const flags: string[] = [];
	if (stream.is_default) flags.push("Default");
	if (stream.is_forced) flags.push("Forced");
	if (stream.is_commentary) flags.push("Commentary");
	if (stream.is_hearing_impaired) flags.push("SDH");
	if (stream.is_cover_art) flags.push("Cover");

	if (flags.length > 0) {
		parts.push(`[${flags.join(", ")}]`);
	}

	return parts.join(" • ");
}

export function BulkStreamCleanupDialog({
	open,
	onOpenChange,
	paths,
	onSuccess,
}: BulkStreamCleanupDialogProps) {
	const [filesData, setFilesData] = useState<Map<string, FileStreamsData>>(
		new Map(),
	);
	const [executing, setExecuting] = useState(false);
	const [queueStatus, setQueueStatus] = useState<QueueStatus | null>(null);
	const [expandedItems, setExpandedItems] = useState<string[]>([]);
	const [completedJobs, setCompletedJobs] = useState<Set<string>>(new Set());
	const [totalJobsSubmitted, setTotalJobsSubmitted] = useState(0);
	const [fakeProgress, setFakeProgress] = useState<Map<string, number>>(
		new Map(),
	);
	const previousRunningPaths = useRef<Set<string>>(new Set());

	// Load streams for all files
	useEffect(() => {
		if (!open || paths.length === 0) return;

		const loadStreams = async () => {
			const newData = new Map<string, FileStreamsData>();

			for (const path of paths) {
				newData.set(path, {
					path,
					streams: null,
					loading: true,
					error: null,
					selectedIndices: new Set(),
				});
			}
			setFilesData(newData);

			// Auto-expand if single file
			if (paths.length === 1) {
				setExpandedItems([paths[0]]);
			} else {
				setExpandedItems([]);
			}

			// Load streams for each file
			for (const path of paths) {
				try {
					const streams = await invoke<MediaStreams>("get_media_streams", {
						path,
					});
					setFilesData((prev) => {
						const next = new Map(prev);
						const fileData = next.get(path);
						if (fileData) {
							fileData.streams = streams;
							fileData.loading = false;
						}
						return next;
					});
				} catch (error) {
					setFilesData((prev) => {
						const next = new Map(prev);
						const fileData = next.get(path);
						if (fileData) {
							fileData.error = String(error);
							fileData.loading = false;
						}
						return next;
					});
				}
			}
		};

		loadStreams();
	}, [open, paths]);

	// Listen for job queue updates
	useEffect(() => {
		if (!executing) return;

		const unlisten = listen<QueueStatus>("job-queue-update", (event) => {
			const currentRunningPaths = new Set(
				event.payload.running.map((j) => j.path),
			);
			const currentQueuedPaths = new Set(
				event.payload.queued.map((j) => j.path),
			);

			// Detect jobs that were running but are no longer running or queued (completed)
			for (const path of previousRunningPaths.current) {
				if (!currentRunningPaths.has(path) && !currentQueuedPaths.has(path)) {
					setCompletedJobs((prev) => new Set(prev).add(path));
					// Clear fake progress for completed job
					setFakeProgress((prev) => {
						const next = new Map(prev);
						next.delete(path);
						return next;
					});
				}
			}

			previousRunningPaths.current = currentRunningPaths;
			setQueueStatus(event.payload);

			// Check if all jobs are done
			const allDone =
				event.payload.running.length === 0 && event.payload.queued.length === 0;
			if (allDone && executing) {
				setExecuting(false);
				toast.success("Bulk stream cleanup completed");
				onSuccess();
				onOpenChange(false);
			}
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	}, [executing, onSuccess, onOpenChange]);

	// Fake progress animation for running jobs
	useEffect(() => {
		if (!executing || !queueStatus) return;

		const interval = setInterval(() => {
			setFakeProgress((prev) => {
				const next = new Map(prev);
				for (const job of queueStatus.running) {
					const current = next.get(job.path) ?? 0;
					// Slowly grow to 60-70% range, slowing down as it approaches
					if (current < 70) {
						const increment = Math.max(0.5, (70 - current) / 20);
						next.set(job.path, Math.min(70, current + increment));
					}
				}
				return next;
			});
		}, 200);

		return () => clearInterval(interval);
	}, [executing, queueStatus]);

	const toggleStream = (path: string, index: number) => {
		setFilesData((prev) => {
			const next = new Map(prev);
			const fileData = next.get(path);
			if (fileData) {
				const newSet = new Set(fileData.selectedIndices);
				if (newSet.has(index)) {
					newSet.delete(index);
				} else {
					newSet.add(index);
				}
				fileData.selectedIndices = newSet;
			}
			return next;
		});
	};

	const applyPreset = useCallback(
		(
			preset: "subtitles" | "non-english-audio" | "commentary" | "cover-art",
		) => {
			setFilesData((prev) => {
				const next = new Map(prev);

				for (const [_path, fileData] of next) {
					if (!fileData.streams) continue;

					const newSet = new Set<number>();

					for (const stream of fileData.streams.streams) {
						let shouldSelect = false;

						switch (preset) {
							case "subtitles":
								shouldSelect = stream.stream_type === "subtitle";
								break;
							case "non-english-audio":
								shouldSelect =
									stream.stream_type === "audio" &&
									stream.language !== "eng" &&
									stream.language !== "en";
								break;
							case "commentary":
								shouldSelect = stream.is_commentary;
								break;
							case "cover-art":
								shouldSelect =
									stream.is_cover_art || stream.stream_type === "attachment";
								break;
						}

						if (shouldSelect) {
							newSet.add(stream.index);
						}
					}

					fileData.selectedIndices = newSet;
				}

				return next;
			});
		},
		[],
	);

	const getTotalSelected = () => {
		let total = 0;
		for (const fileData of filesData.values()) {
			total += fileData.selectedIndices.size;
		}
		return total;
	};

	const handleExecute = async () => {
		const operations: StreamRemovalOp[] = [];

		for (const [path, fileData] of filesData) {
			if (fileData.selectedIndices.size > 0) {
				operations.push({
					path,
					stream_indices: Array.from(fileData.selectedIndices),
				});
			}
		}

		if (operations.length === 0) {
			toast.error("No streams selected for removal");
			return;
		}

		try {
			setExecuting(true);
			setCompletedJobs(new Set());
			setFakeProgress(new Map());
			setTotalJobsSubmitted(operations.length);
			previousRunningPaths.current = new Set();

			const result = await invoke<BulkStreamRemovalResult>(
				"bulk_remove_streams",
				{
					operations,
					overwrite: true, // Always overwrite for bulk operations
				},
			);

			if (result.errors.length > 0) {
				console.error("Bulk stream removal errors:", result.errors);
				toast.error(`${result.errors.length} file(s) failed to queue`);
			}

			if (result.jobs_queued > 0) {
				toast.success(`Queued ${result.jobs_queued} job(s)`);
			} else {
				setExecuting(false);
				toast.error("No jobs were queued");
			}
		} catch (error) {
			setExecuting(false);
			toast.error(`Failed to queue jobs: ${error}`);
		}
	};

	const totalSelected = getTotalSelected();
	const hasAnyLoading = Array.from(filesData.values()).some((f) => f.loading);

	// Build display list for all jobs
	const getJobDisplayList = (): JobDisplayInfo[] => {
		if (!queueStatus) return [];

		const jobs: JobDisplayInfo[] = [];

		// Add completed jobs first
		for (const path of completedJobs) {
			jobs.push({
				path,
				state: "completed",
				progress: 100,
				fileName: path.split("/").pop() || path,
			});
		}

		// Add running jobs
		for (const job of queueStatus.running) {
			if (!completedJobs.has(job.path)) {
				// Use fake progress since stream removal doesn't report real progress
				const progress = fakeProgress.get(job.path) ?? 0;
				jobs.push({
					path: job.path,
					state: "running",
					progress,
					fileName: job.path.split("/").pop() || job.path,
				});
			}
		}

		// Add queued jobs
		for (const job of queueStatus.queued) {
			if (!completedJobs.has(job.path)) {
				jobs.push({
					path: job.path,
					state: "queued",
					progress: 0,
					fileName: job.path.split("/").pop() || job.path,
				});
			}
		}

		return jobs;
	};

	const completedCount = completedJobs.size;

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="flex h-[80vh] w-full flex-col overflow-hidden sm:max-w-2xl">
				<DialogHeader>
					<DialogTitle>Clean Streams</DialogTitle>
					<DialogDescription>
						Select streams to remove from {paths.length} file(s)
					</DialogDescription>
				</DialogHeader>

				<div className="flex min-h-0 flex-1 flex-col gap-y-4 overflow-hidden">
					{!executing && (
						<div className="flex flex-wrap gap-2">
							<Button
								size="sm"
								variant="outline"
								onClick={() => applyPreset("subtitles")}
								disabled={hasAnyLoading}
							>
								All Subtitles
							</Button>
							<Button
								size="sm"
								variant="outline"
								onClick={() => applyPreset("non-english-audio")}
								disabled={hasAnyLoading}
							>
								Non-English Audio
							</Button>
							<Button
								size="sm"
								variant="outline"
								onClick={() => applyPreset("commentary")}
								disabled={hasAnyLoading}
							>
								Commentary
							</Button>
							<Button
								size="sm"
								variant="outline"
								onClick={() => applyPreset("cover-art")}
								disabled={hasAnyLoading}
							>
								Cover Art
							</Button>
						</div>
					)}

					<div className="min-h-0 flex-1 overflow-hidden">
						<ScrollArea className="h-full">
							{executing ? (
								<div className="space-y-4">
									<div className="flex items-center justify-between">
										<p className="text-muted-foreground text-sm">
											{queueStatus?.running.length ?? 0} running
											{(queueStatus?.queued.length ?? 0) > 0 &&
												`, ${queueStatus?.queued.length} queued`}
										</p>
										{totalJobsSubmitted > 0 && (
											<p className="text-muted-foreground text-sm">
												{Math.max(0, completedCount)}/{totalJobsSubmitted}{" "}
												completed
											</p>
										)}
									</div>
									<div className="space-y-2">
										{getJobDisplayList().map((job) => (
											<div
												key={job.path}
												className={`space-y-1 rounded-md border p-2 transition-all ${
													job.state === "completed"
														? "border-green-500/50 bg-green-500/10"
														: job.state === "running"
															? "border-primary/50 bg-primary/5"
															: "border-muted bg-muted/30"
												}`}
											>
												<div className="flex items-center justify-between gap-2">
													<div className="flex min-w-0 flex-1 items-center gap-2">
														{job.state === "completed" ? (
															<Check className="size-4 shrink-0 text-green-500" />
														) : job.state === "running" ? (
															<Loader2 className="size-4 shrink-0 animate-spin text-primary" />
														) : (
															<Clock className="size-4 shrink-0 text-muted-foreground" />
														)}
														<p className="flex-1 truncate font-medium text-sm">
															{job.fileName}
														</p>
													</div>
													<span className="shrink-0 text-muted-foreground text-xs">
														{job.state === "completed"
															? "Done"
															: job.state === "queued"
																? "Waiting..."
																: `${Math.round(job.progress)}%`}
													</span>
												</div>
												<div className="h-1.5 w-full overflow-hidden rounded-full bg-secondary">
													{job.state === "completed" ? (
														<div className="h-1.5 w-full rounded-full bg-green-500 transition-all" />
													) : job.state === "running" ? (
														<div
															className="h-1.5 rounded-full bg-primary transition-all"
															style={{ width: `${job.progress}%` }}
														/>
													) : (
														<div className="h-1.5 w-full animate-pulse rounded-full bg-muted-foreground/20" />
													)}
												</div>
											</div>
										))}
									</div>
								</div>
							) : (
								<Accordion
									type="multiple"
									value={expandedItems}
									onValueChange={setExpandedItems}
									className="w-full space-y-2"
								>
									{Array.from(filesData.values()).map((fileData) => (
										<AccordionItem
											key={fileData.path}
											value={fileData.path}
											className="rounded-lg border px-3 last:border-b"
										>
											<AccordionTrigger className="gap-2 py-3 hover:no-underline">
												<div className="flex flex-1 items-center gap-2">
													<span className="flex-1 truncate text-left font-medium text-sm">
														{fileData.path.split("/").pop()}
													</span>
													{fileData.loading && (
														<Loader2 className="size-4 animate-spin" />
													)}
													{fileData.selectedIndices.size > 0 && (
														<span className="text-muted-foreground text-xs">
															{fileData.selectedIndices.size} selected
														</span>
													)}
												</div>
											</AccordionTrigger>
											<AccordionContent>
												<div className="space-y-2 pb-2">
													{fileData.loading && (
														<p className="text-muted-foreground text-sm">
															Loading streams...
														</p>
													)}
													{fileData.error && (
														<p className="text-destructive text-sm">
															{fileData.error}
														</p>
													)}
													{fileData.streams && (
														<div className="space-y-1">
															{fileData.streams.streams.map((stream) => (
																<div
																	key={stream.index}
																	className="flex items-center gap-2"
																>
																	<Checkbox
																		id={`stream-${fileData.path}-${stream.index}`}
																		checked={fileData.selectedIndices.has(
																			stream.index,
																		)}
																		onCheckedChange={() =>
																			toggleStream(fileData.path, stream.index)
																		}
																	/>
																	<label
																		htmlFor={`stream-${fileData.path}-${stream.index}`}
																		className="flex-1 cursor-pointer text-sm"
																	>
																		<span className="mr-2 font-mono text-muted-foreground text-xs">
																			#{stream.index}
																		</span>
																		{formatStreamLabel(stream)}
																	</label>
																</div>
															))}
														</div>
													)}
												</div>
											</AccordionContent>
										</AccordionItem>
									))}
								</Accordion>
							)}
						</ScrollArea>
					</div>
				</div>

				<DialogFooter>
					{executing ? (
						<Button variant="outline" onClick={() => onOpenChange(false)}>
							Close
						</Button>
					) : (
						<>
							<Button variant="outline" onClick={() => onOpenChange(false)}>
								Cancel
							</Button>
							<Button
								onClick={handleExecute}
								disabled={totalSelected === 0 || hasAnyLoading}
							>
								<Trash2 className="mr-2 size-4" />
								Remove {totalSelected} Stream{totalSelected !== 1 ? "s" : ""}
							</Button>
						</>
					)}
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
