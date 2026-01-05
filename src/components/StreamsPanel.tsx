import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
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
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useFileBrowserStore } from "@/stores/fileBrowserStore";
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
}

interface StreamRemovalResult {
	success: boolean;
	output_path: string;
	message: string;
}

interface StreamsPanelProps {
	filePath: string | null;
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
	tha: "Thai",
	vie: "Vietnamese",
	pol: "Polish",
	nld: "Dutch",
	swe: "Swedish",
	nor: "Norwegian",
	dan: "Danish",
	fin: "Finnish",
	tur: "Turkish",
	heb: "Hebrew",
	cze: "Czech",
	hun: "Hungarian",
	ron: "Romanian",
	bul: "Bulgarian",
	hrv: "Croatian",
	srp: "Serbian",
	slv: "Slovenian",
	ukr: "Ukrainian",
	ell: "Greek",
	gre: "Greek",
	ind: "Indonesian",
	may: "Malay",
	msa: "Malay",
	tam: "Tamil",
	tel: "Telugu",
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
		case "data":
			return "📊";
		default:
			return "❓";
	}
}

function formatSize(bytes: number): string {
	if (bytes === 0) return "0 B";
	const units = ["B", "KB", "MB", "GB"];
	const i = Math.floor(Math.log(bytes) / Math.log(1024));
	return `${(bytes / 1024 ** i).toFixed(2)} ${units[i]}`;
}

function getStreamTypeBadgeVariant(
	type: StreamType,
): "default" | "secondary" | "outline" | "destructive" {
	switch (type) {
		case "video":
			return "default";
		case "audio":
			return "secondary";
		case "subtitle":
			return "outline";
		default:
			return "outline";
	}
}

function StreamCard({
	stream,
	isSelected,
	onToggle,
}: {
	stream: StreamInfo;
	isSelected: boolean;
	onToggle: () => void;
}) {
	const badges: {
		label: string;
		variant: "default" | "secondary" | "outline";
	}[] = [];

	if (stream.is_cover_art)
		badges.push({ label: "Cover Art", variant: "secondary" });
	if (stream.is_default) badges.push({ label: "Default", variant: "default" });
	if (stream.is_forced) badges.push({ label: "Forced", variant: "secondary" });
	if (stream.is_hearing_impaired)
		badges.push({ label: "SDH", variant: "outline" });
	if (stream.is_visual_impaired)
		badges.push({ label: "VI", variant: "outline" });
	if (stream.is_commentary)
		badges.push({ label: "Commentary", variant: "outline" });
	if (stream.is_lyrics) badges.push({ label: "Lyrics", variant: "outline" });
	if (stream.is_karaoke) badges.push({ label: "Karaoke", variant: "outline" });

	const getDetails = () => {
		const details: string[] = [];

		if (stream.stream_type === "video") {
			if (stream.width && stream.height) {
				details.push(`${stream.width}x${stream.height}`);
			}
			if (stream.frame_rate) {
				const parts = stream.frame_rate.split("/");
				if (parts.length === 2) {
					const fps = Math.round(
						parseInt(parts[0], 10) / parseInt(parts[1], 10),
					);
					details.push(`${fps}fps`);
				}
			}
			if (stream.pixel_format) {
				details.push(stream.pixel_format);
			}
		} else if (stream.stream_type === "audio") {
			if (stream.channels) {
				const channelStr =
					stream.channels === 1
						? "Mono"
						: stream.channels === 2
							? "Stereo"
							: `${stream.channels}ch`;
				details.push(channelStr);
			}
			if (stream.sample_rate) {
				details.push(`${parseInt(stream.sample_rate, 10) / 1000}kHz`);
			}
			if (stream.bit_rate) {
				details.push(`${Math.round(parseInt(stream.bit_rate, 10) / 1000)}kbps`);
			}
		} else if (stream.stream_type === "subtitle") {
			if (stream.subtitle_format) {
				details.push(stream.subtitle_format.toUpperCase());
			}
		}

		return details.join(" • ");
	};

	return (
		<div
			className={`rounded-lg border p-3 transition-colors ${
				isSelected
					? "border-destructive bg-destructive/10"
					: "border-border bg-card hover:bg-accent/50"
			}`}
		>
			<div className="flex items-start gap-3">
				<Checkbox
					checked={isSelected}
					onCheckedChange={onToggle}
					className="mt-1"
				/>
				<div className="min-w-0 flex-1">
					<div className="mb-1 flex flex-wrap items-center gap-2">
						<span className="text-lg" aria-hidden="true">
							{getStreamTypeIcon(stream.stream_type, stream.is_cover_art)}
						</span>
						<Badge variant={getStreamTypeBadgeVariant(stream.stream_type)}>
							{stream.is_cover_art
								? "Cover"
								: stream.stream_type.charAt(0).toUpperCase() +
									stream.stream_type.slice(1)}
						</Badge>
						{stream.stream_type !== "video" && (
							<span className="font-medium text-sm">
								#{stream.index} • {getLanguageName(stream.language)}
							</span>
						)}
						{stream.estimated_size && (
							<span className="text-muted-foreground text-xs">
								({formatSize(stream.estimated_size)})
							</span>
						)}
					</div>

					{stream.title && (
						<p className="mb-1 truncate text-muted-foreground text-sm">
							{stream.title}
						</p>
					)}

					<p className="text-muted-foreground text-xs">
						{stream.codec_name?.toUpperCase() || "Unknown codec"}
						{getDetails() && ` • ${getDetails()}`}
					</p>

					{badges.length > 0 && (
						<div className="mt-2 flex flex-wrap gap-1">
							{badges.map((badge) => (
								<Badge
									key={badge.label}
									variant={badge.variant}
									className="text-xs"
								>
									{badge.label}
								</Badge>
							))}
						</div>
					)}
				</div>
			</div>
		</div>
	);
}

function StreamSection({
	title,
	icon,
	streams,
	selectedStreams,
	onToggleStream,
}: {
	title: string;
	icon: string;
	streams: StreamInfo[];
	selectedStreams: Set<number>;
	onToggleStream: (index: number) => void;
}) {
	if (streams.length === 0) return null;

	return (
		<div className="mb-4">
			<h3 className="mb-2 flex items-center gap-2 font-semibold text-sm">
				<span>{icon}</span>
				{title} ({streams.length})
			</h3>
			<div className="space-y-2">
				{streams.map((stream) => (
					<StreamCard
						key={stream.index}
						stream={stream}
						isSelected={selectedStreams.has(stream.index)}
						onToggle={() => onToggleStream(stream.index)}
					/>
				))}
			</div>
		</div>
	);
}

export function StreamsPanel({ filePath }: StreamsPanelProps) {
	const refresh = useFileBrowserStore((state) => state.refresh);
	const [mediaStreams, setMediaStreams] = useState<MediaStreams | null>(null);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [selectedStreams, setSelectedStreams] = useState<Set<number>>(
		new Set(),
	);
	const [removing, setRemoving] = useState(false);
	const [result, setResult] = useState<StreamRemovalResult | null>(null);
	const [showConfirmDialog, setShowConfirmDialog] = useState(false);
	const [queueStatus, setQueueStatus] = useState<QueueStatus | null>(null);

	// Listen for queue updates
	useEffect(() => {
		const unlisten = listen<QueueStatus>("job-queue-update", (event) => {
			setQueueStatus(event.payload);
		});

		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	useEffect(() => {
		if (!filePath) {
			setMediaStreams(null);
			setSelectedStreams(new Set());
			setResult(null);
			return;
		}

		const loadStreams = async () => {
			setLoading(true);
			setError(null);
			setSelectedStreams(new Set());
			setResult(null);
			try {
				const data = await invoke<MediaStreams>("get_media_streams", {
					path: filePath,
				});
				setMediaStreams(data);
			} catch (e) {
				setError(String(e));
				setMediaStreams(null);
			} finally {
				setLoading(false);
			}
		};

		loadStreams();
	}, [filePath]);

	const toggleStream = (index: number) => {
		setSelectedStreams((prev) => {
			const next = new Set(prev);
			if (next.has(index)) {
				next.delete(index);
			} else {
				next.add(index);
			}
			return next;
		});
	};

	const handleRemoveStreams = async (overwrite: boolean) => {
		if (!filePath || selectedStreams.size === 0) return;

		setShowConfirmDialog(false);
		setRemoving(true);
		setResult(null);
		setError(null);
		try {
			const res = await invoke<StreamRemovalResult>("remove_streams", {
				path: filePath,
				streamIndices: Array.from(selectedStreams),
				overwrite,
			});
			setResult(res);
			setSelectedStreams(new Set());
			// Reload streams and file list
			const data = await invoke<MediaStreams>("get_media_streams", {
				path: overwrite ? filePath : res.output_path,
			});
			setMediaStreams(data);
			refresh();
		} catch (e) {
			const errorMessage = String(e);
			// Don't show error if job was queued or already exists
			if (
				errorMessage.includes("already queued") ||
				errorMessage.includes("already in progress")
			) {
				console.log("[StreamsPanel] Job queued or already running");
				// Keep removing state to show progress
			} else {
				setError(errorMessage);
			}
		} finally {
			setRemoving(false);
		}
	};

	const openConfirmDialog = () => {
		setShowConfirmDialog(true);
	};

	const selectAllSubtitles = () => {
		if (!mediaStreams) return;
		const subtitleIndices = mediaStreams.streams
			.filter((s) => s.stream_type === "subtitle")
			.map((s) => s.index);
		setSelectedStreams(new Set(subtitleIndices));
	};

	const selectNonEnglishSubtitles = () => {
		if (!mediaStreams) return;
		const indices = mediaStreams.streams
			.filter(
				(s) =>
					s.stream_type === "subtitle" &&
					s.language?.toLowerCase() !== "eng" &&
					s.language?.toLowerCase() !== "en",
			)
			.map((s) => s.index);
		setSelectedStreams(new Set(indices));
	};

	const clearSelection = () => {
		setSelectedStreams(new Set());
	};

	// Check if there's a stream removal job for this file
	const streamRemovalJob =
		queueStatus?.queued.find(
			(job) => job.path === filePath && job.state.includes("stream_removal"),
		) ||
		queueStatus?.running.find(
			(job) => job.path === filePath && job.state.includes("stream_removal"),
		);

	const isJobQueued = streamRemovalJob?.state.startsWith("queued:");
	const isJobRunning = streamRemovalJob?.state.startsWith("running:");

	if (!filePath) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<p>Select a media file to view streams</p>
			</div>
		);
	}

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<p>Loading streams...</p>
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex h-full items-center justify-center p-4 text-destructive">
				<p className="text-center">Error: {error}</p>
			</div>
		);
	}

	if (!mediaStreams || mediaStreams.streams.length === 0) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<p>No streams found in this file</p>
			</div>
		);
	}

	const videoStreams = mediaStreams.streams.filter(
		(s) => s.stream_type === "video" && !s.is_cover_art,
	);
	const coverArtStreams = mediaStreams.streams.filter((s) => s.is_cover_art);
	const audioStreams = mediaStreams.streams.filter(
		(s) => s.stream_type === "audio",
	);
	const subtitleStreams = mediaStreams.streams.filter(
		(s) => s.stream_type === "subtitle",
	);
	const otherStreams = mediaStreams.streams.filter(
		(s) =>
			s.stream_type !== "video" &&
			s.stream_type !== "audio" &&
			s.stream_type !== "subtitle" &&
			!s.is_cover_art,
	);

	// Calculate estimated savings
	const estimatedSavings = Array.from(selectedStreams).reduce((total, idx) => {
		const stream = mediaStreams.streams.find((s) => s.index === idx);
		return total + (stream?.estimated_size || 0);
	}, 0);

	return (
		<div className="flex h-full flex-col">
			<ScrollArea className="h-[calc(100%-10rem)] flex-1">
				<div className="p-4">
					<div className="mb-4 flex items-center justify-between">
						<h2 className="font-semibold text-lg">Streams</h2>
						<span className="text-muted-foreground text-sm">
							{mediaStreams.streams.length} total
						</span>
					</div>

					{result && (
						<div className="mb-4 rounded-lg border border-green-500/30 bg-green-500/10 p-3 text-green-700 dark:text-green-400">
							<p className="font-medium text-sm">✓ {result.message}</p>
						</div>
					)}

					{isJobQueued && (
						<div className="mb-4 rounded-lg border border-blue-500/30 bg-blue-500/10 p-3 text-blue-700 dark:text-blue-400">
							<p className="font-medium text-sm">
								⏳ Stream removal queued - waiting for available slot
							</p>
						</div>
					)}

					{isJobRunning && (
						<div className="mb-4 rounded-lg border border-blue-500/30 bg-blue-500/10 p-3 text-blue-700 dark:text-blue-400">
							<div className="flex items-center justify-between">
								<p className="font-medium text-sm">
									🔄 {streamRemovalJob?.progress_stage || "Removing streams..."}
								</p>
								{streamRemovalJob?.progress_percentage !== undefined && (
									<span className="text-xs">
										{streamRemovalJob.progress_percentage.toFixed(0)}%
									</span>
								)}
							</div>
							{streamRemovalJob?.progress_percentage !== undefined && (
								<div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-blue-500/20">
									<div
										className="h-full bg-blue-500 transition-all"
										style={{
											width: `${streamRemovalJob.progress_percentage}%`,
										}}
									/>
								</div>
							)}
							<p className="mt-1 text-xs opacity-75">
								Running for {streamRemovalJob?.running_seconds?.toFixed(1)}s
							</p>
						</div>
					)}

					{subtitleStreams.length > 0 && (
						<div className="mb-4 flex flex-wrap gap-2">
							<Button variant="outline" size="sm" onClick={selectAllSubtitles}>
								Select All Subtitles
							</Button>
							<Button
								variant="outline"
								size="sm"
								onClick={selectNonEnglishSubtitles}
							>
								Select Non-English Subs
							</Button>
							{selectedStreams.size > 0 && (
								<Button variant="ghost" size="sm" onClick={clearSelection}>
									Clear Selection
								</Button>
							)}
						</div>
					)}

					<StreamSection
						title="Video"
						icon="🎬"
						streams={videoStreams}
						selectedStreams={selectedStreams}
						onToggleStream={toggleStream}
					/>

					<StreamSection
						title="Audio"
						icon="🔊"
						streams={audioStreams}
						selectedStreams={selectedStreams}
						onToggleStream={toggleStream}
					/>

					<StreamSection
						title="Subtitles"
						icon="💬"
						streams={subtitleStreams}
						selectedStreams={selectedStreams}
						onToggleStream={toggleStream}
					/>

					{coverArtStreams.length > 0 && (
						<StreamSection
							title="Cover Art"
							icon="🖼️"
							streams={coverArtStreams}
							selectedStreams={selectedStreams}
							onToggleStream={toggleStream}
						/>
					)}

					{otherStreams.length > 0 && (
						<StreamSection
							title="Other"
							icon="📎"
							streams={otherStreams}
							selectedStreams={selectedStreams}
							onToggleStream={toggleStream}
						/>
					)}
				</div>
			</ScrollArea>

			{selectedStreams.size > 0 && (
				<div className="border-border border-t bg-background p-4">
					{estimatedSavings > 0 && (
						<p className="mb-2 text-center font-medium text-green-600 text-sm dark:text-green-400">
							Estimated savings: ~{formatSize(estimatedSavings)}
						</p>
					)}
					<Button
						variant="destructive"
						className="w-full"
						onClick={openConfirmDialog}
						disabled={removing}
					>
						{removing
							? "Removing..."
							: `Remove ${selectedStreams.size} Selected Stream${selectedStreams.size > 1 ? "s" : ""}`}
					</Button>
					<p className="mt-2 text-center text-muted-foreground text-xs">
						Choose to overwrite original or create a new file
					</p>
				</div>
			)}

			<AlertDialog open={showConfirmDialog} onOpenChange={setShowConfirmDialog}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Remove Streams</AlertDialogTitle>
						<AlertDialogDescription>
							How would you like to save the changes? You can either overwrite
							the original file or create a new file with "_modified" suffix.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter className="flex-col gap-2 sm:flex-row">
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction
							onClick={() => handleRemoveStreams(false)}
							className="bg-secondary text-secondary-foreground hover:bg-secondary/80"
						>
							Create New File
						</AlertDialogAction>
						<AlertDialogAction
							onClick={() => handleRemoveStreams(true)}
							className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
						>
							Overwrite Original
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</div>
	);
}
