import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ChevronDown, ChevronRight, Loader2, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
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
	expanded: boolean;
	selectedIndices: Set<number>;
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
					expanded: paths.length === 1, // Auto-expand if single file
					selectedIndices: new Set(),
				});
			}
			setFilesData(newData);

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

	const toggleExpanded = (path: string) => {
		setFilesData((prev) => {
			const next = new Map(prev);
			const fileData = next.get(path);
			if (fileData) {
				fileData.expanded = !fileData.expanded;
			}
			return next;
		});
	};

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

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-4xl max-h-[80vh] flex flex-col">
				<DialogHeader>
					<DialogTitle>Clean Streams</DialogTitle>
					<DialogDescription>
						Select streams to remove from {paths.length} file(s)
					</DialogDescription>
				</DialogHeader>

				{!executing && (
					<div className="flex gap-2 flex-wrap">
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

				<ScrollArea className="flex-1 pr-4">
					{executing ? (
						<div className="space-y-4">
							<p className="text-sm text-muted-foreground">
								Processing {queueStatus?.running.length ?? 0} running,{" "}
								{queueStatus?.queued.length ?? 0} queued
							</p>
							{queueStatus && (
								<div className="space-y-2">
									{queueStatus.running.map((job) => (
										<div key={job.path} className="space-y-1">
											<div className="flex items-center justify-between">
												<p className="text-sm font-medium truncate flex-1">
													{job.path.split("/").pop()}
												</p>
												{job.progress_percentage !== null &&
													job.progress_percentage !== undefined && (
														<span className="text-xs text-muted-foreground ml-2">
															{Math.round(job.progress_percentage)}%
														</span>
													)}
											</div>
											{job.progress_percentage !== null && (
												<div className="w-full bg-secondary rounded-full h-1.5">
													<div
														className="bg-primary h-1.5 rounded-full transition-all"
														style={{ width: `${job.progress_percentage}%` }}
													/>
												</div>
											)}
										</div>
									))}
								</div>
							)}
						</div>
					) : (
						<div className="space-y-2">
							{Array.from(filesData.values()).map((fileData) => (
								<div key={fileData.path} className="border rounded-lg p-3">
									<div
										className="flex items-center gap-2 cursor-pointer"
										onClick={() => toggleExpanded(fileData.path)}
									>
										{fileData.expanded ? (
											<ChevronDown className="h-4 w-4" />
										) : (
											<ChevronRight className="h-4 w-4" />
										)}
										<span className="font-medium flex-1 truncate text-sm">
											{fileData.path.split("/").pop()}
										</span>
										{fileData.loading && (
											<Loader2 className="h-4 w-4 animate-spin" />
										)}
										{fileData.selectedIndices.size > 0 && (
											<span className="text-xs text-muted-foreground">
												{fileData.selectedIndices.size} selected
											</span>
										)}
									</div>

									{fileData.expanded && (
										<div className="mt-3 ml-6 space-y-2">
											{fileData.loading && (
												<p className="text-sm text-muted-foreground">
													Loading streams...
												</p>
											)}
											{fileData.error && (
												<p className="text-sm text-destructive">
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
																checked={fileData.selectedIndices.has(
																	stream.index,
																)}
																onCheckedChange={() =>
																	toggleStream(fileData.path, stream.index)
																}
															/>
															<label className="text-sm cursor-pointer flex-1">
																<span className="font-mono text-xs text-muted-foreground mr-2">
																	#{stream.index}
																</span>
																{formatStreamLabel(stream)}
															</label>
														</div>
													))}
												</div>
											)}
										</div>
									)}
								</div>
							))}
						</div>
					)}
				</ScrollArea>

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
								<Trash2 className="h-4 w-4 mr-2" />
								Remove {totalSelected} Stream{totalSelected !== 1 ? "s" : ""}
							</Button>
						</>
					)}
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
