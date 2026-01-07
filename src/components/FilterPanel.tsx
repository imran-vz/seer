import { invoke } from "@tauri-apps/api/core";
import { Calendar, FileType, Filter, HardDrive, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

type FileTypeFilter = "all" | "files_only" | "directories_only" | "media_only";

interface MediaFilters {
	video_codecs: string[];
	audio_codecs: string[];
	min_width: number | null;
	max_width: number | null;
	min_height: number | null;
	max_height: number | null;
	min_duration: number | null;
	max_duration: number | null;
}

interface FilterCriteria {
	size_min: number | null;
	size_max: number | null;
	date_min: string | null;
	date_max: string | null;
	extensions: string[];
	media_filters: MediaFilters | null;
	file_type: FileTypeFilter | null;
}

interface FilteredFileEntry {
	name: string;
	path: string;
	is_dir: boolean;
	is_media: boolean;
	size: number;
	modified: string | null;
	video_codec: string | null;
	audio_codec: string | null;
	width: number | null;
	height: number | null;
	duration: number | null;
}

interface FilterResult {
	files: FilteredFileEntry[];
	total_count: number;
	filtered_count: number;
}

interface FilterPanelProps {
	currentPath: string;
	isOpen: boolean;
	onToggle: () => void;
	onFilterApplied: (result: FilterResult | null) => void;
}

const COMMON_VIDEO_CODECS = ["h264", "hevc", "h265", "av1", "vp9", "mpeg4"];
const COMMON_AUDIO_CODECS = [
	"aac",
	"ac3",
	"eac3",
	"dts",
	"flac",
	"opus",
	"mp3",
];
const COMMON_EXTENSIONS = [
	"mp4",
	"mkv",
	"avi",
	"mov",
	"mp3",
	"flac",
	"wav",
	"jpg",
	"png",
	"pdf",
	"txt",
];

const SIZE_PRESETS = [
	{ label: "> 100MB", min: 100 * 1024 * 1024, max: null },
	{ label: "> 1GB", min: 1024 * 1024 * 1024, max: null },
	{ label: "< 10MB", min: null, max: 10 * 1024 * 1024 },
	{ label: "100MB - 1GB", min: 100 * 1024 * 1024, max: 1024 * 1024 * 1024 },
];

const RESOLUTION_PRESETS = [
	{ label: "4K+", minWidth: 3840, minHeight: 2160 },
	{ label: "1080p+", minWidth: 1920, minHeight: 1080 },
	{ label: "720p+", minWidth: 1280, minHeight: 720 },
];

const emptyFilters: FilterCriteria = {
	size_min: null,
	size_max: null,
	date_min: null,
	date_max: null,
	extensions: [],
	media_filters: null,
	file_type: null,
};

function hasActiveFilters(filters: FilterCriteria): boolean {
	return (
		filters.size_min !== null ||
		filters.size_max !== null ||
		filters.date_min !== null ||
		filters.date_max !== null ||
		filters.extensions.length > 0 ||
		filters.file_type !== null ||
		filters.media_filters !== null
	);
}

export function FilterPanel({
	currentPath,
	isOpen,
	onToggle,
	onFilterApplied,
}: FilterPanelProps) {
	const [filters, setFilters] = useState<FilterCriteria>(emptyFilters);
	const [availableExtensions, setAvailableExtensions] = useState<string[]>([]);
	const [loading, setLoading] = useState(false);
	const [showAdvanced, setShowAdvanced] = useState(false);

	// Fetch available extensions when path changes
	useEffect(() => {
		if (!currentPath) return;
		invoke<string[]>("get_available_extensions", { currentDir: currentPath })
			.then(setAvailableExtensions)
			.catch(console.error);
	}, [currentPath]);

	const applyFilters = useCallback(async () => {
		if (!currentPath) return;

		setLoading(true);
		try {
			const result = await invoke<FilterResult>("apply_filters", {
				currentDir: currentPath,
				filters,
			});
			onFilterApplied(result);
		} catch (err) {
			console.error("Filter error:", err);
		} finally {
			setLoading(false);
		}
	}, [currentPath, filters, onFilterApplied]);

	const clearFilters = useCallback(() => {
		setFilters(emptyFilters);
		onFilterApplied(null);
	}, [onFilterApplied]);

	const toggleExtension = (ext: string) => {
		setFilters((prev) => ({
			...prev,
			extensions: prev.extensions.includes(ext)
				? prev.extensions.filter((e) => e !== ext)
				: [...prev.extensions, ext],
		}));
	};

	const toggleVideoCodec = (codec: string) => {
		setFilters((prev) => {
			const currentCodecs = prev.media_filters?.video_codecs ?? [];
			const newCodecs = currentCodecs.includes(codec)
				? currentCodecs.filter((c) => c !== codec)
				: [...currentCodecs, codec];

			if (
				newCodecs.length === 0 &&
				(prev.media_filters?.audio_codecs?.length ?? 0) === 0 &&
				prev.media_filters?.min_width === null &&
				prev.media_filters?.min_height === null &&
				prev.media_filters?.min_duration === null
			) {
				return { ...prev, media_filters: null };
			}

			return {
				...prev,
				media_filters: {
					video_codecs: newCodecs,
					audio_codecs: prev.media_filters?.audio_codecs ?? [],
					min_width: prev.media_filters?.min_width ?? null,
					max_width: prev.media_filters?.max_width ?? null,
					min_height: prev.media_filters?.min_height ?? null,
					max_height: prev.media_filters?.max_height ?? null,
					min_duration: prev.media_filters?.min_duration ?? null,
					max_duration: prev.media_filters?.max_duration ?? null,
				},
			};
		});
	};

	const toggleAudioCodec = (codec: string) => {
		setFilters((prev) => {
			const currentCodecs = prev.media_filters?.audio_codecs ?? [];
			const newCodecs = currentCodecs.includes(codec)
				? currentCodecs.filter((c) => c !== codec)
				: [...currentCodecs, codec];

			if (
				newCodecs.length === 0 &&
				(prev.media_filters?.video_codecs?.length ?? 0) === 0 &&
				prev.media_filters?.min_width === null &&
				prev.media_filters?.min_height === null &&
				prev.media_filters?.min_duration === null
			) {
				return { ...prev, media_filters: null };
			}

			return {
				...prev,
				media_filters: {
					video_codecs: prev.media_filters?.video_codecs ?? [],
					audio_codecs: newCodecs,
					min_width: prev.media_filters?.min_width ?? null,
					max_width: prev.media_filters?.max_width ?? null,
					min_height: prev.media_filters?.min_height ?? null,
					max_height: prev.media_filters?.max_height ?? null,
					min_duration: prev.media_filters?.min_duration ?? null,
					max_duration: prev.media_filters?.max_duration ?? null,
				},
			};
		});
	};

	const setResolutionPreset = (minWidth: number, minHeight: number) => {
		setFilters((prev) => ({
			...prev,
			media_filters: {
				video_codecs: prev.media_filters?.video_codecs ?? [],
				audio_codecs: prev.media_filters?.audio_codecs ?? [],
				min_width: minWidth,
				max_width: null,
				min_height: minHeight,
				max_height: null,
				min_duration: prev.media_filters?.min_duration ?? null,
				max_duration: prev.media_filters?.max_duration ?? null,
			},
		}));
	};

	const setSizePreset = (min: number | null, max: number | null) => {
		setFilters((prev) => ({
			...prev,
			size_min: min,
			size_max: max,
		}));
	};

	const activeFilterCount = [
		filters.size_min !== null || filters.size_max !== null,
		filters.date_min !== null || filters.date_max !== null,
		filters.extensions.length > 0,
		filters.file_type !== null,
		filters.media_filters !== null,
	].filter(Boolean).length;

	const isFiltersActive = hasActiveFilters(filters);

	return (
		<div className="border-border/50 border-b">
			{/* Toggle Bar */}
			<div className="flex items-center gap-2 px-2 py-1">
				<Button
					variant={isOpen ? "secondary" : "ghost"}
					size="sm"
					onClick={onToggle}
					className="h-7 text-xs"
				>
					<Filter className="mr-1.5 h-3.5 w-3.5" />
					Filter
					{activeFilterCount > 0 && (
						<Badge variant="default" className="ml-1.5 h-4 px-1.5 text-[10px]">
							{activeFilterCount}
						</Badge>
					)}
				</Button>

				{isFiltersActive && (
					<>
						<div className="flex flex-wrap gap-1">
							{(filters.size_min || filters.size_max) && (
								<Badge variant="outline" className="h-5 text-[10px]">
									<HardDrive className="mr-1 h-3 w-3" />
									Size
									<button
										type="button"
										onClick={() =>
											setFilters((f) => ({
												...f,
												size_min: null,
												size_max: null,
											}))
										}
										className="ml-1 rounded-full hover:bg-muted"
									>
										<X className="h-3 w-3" />
									</button>
								</Badge>
							)}
							{(filters.date_min || filters.date_max) && (
								<Badge variant="outline" className="h-5 text-[10px]">
									<Calendar className="mr-1 h-3 w-3" />
									Date
									<button
										type="button"
										onClick={() =>
											setFilters((f) => ({
												...f,
												date_min: null,
												date_max: null,
											}))
										}
										className="ml-1 rounded-full hover:bg-muted"
									>
										<X className="h-3 w-3" />
									</button>
								</Badge>
							)}
							{filters.extensions.length > 0 && (
								<Badge variant="outline" className="h-5 text-[10px]">
									<FileType className="mr-1 h-3 w-3" />
									{filters.extensions.length} ext
									<button
										type="button"
										onClick={() =>
											setFilters((f) => ({ ...f, extensions: [] }))
										}
										className="ml-1 rounded-full hover:bg-muted"
									>
										<X className="h-3 w-3" />
									</button>
								</Badge>
							)}
							{filters.media_filters && (
								<Badge variant="outline" className="h-5 text-[10px]">
									Media
									<button
										type="button"
										onClick={() =>
											setFilters((f) => ({ ...f, media_filters: null }))
										}
										className="ml-1 rounded-full hover:bg-muted"
									>
										<X className="h-3 w-3" />
									</button>
								</Badge>
							)}
						</div>
						<Button
							variant="ghost"
							size="sm"
							onClick={clearFilters}
							className="h-6 px-2 text-[10px] text-muted-foreground"
						>
							Clear all
						</Button>
					</>
				)}
			</div>

			{/* Filter Panel */}
			{isOpen && (
				<div className="space-y-3 border-border/30 border-t bg-muted/20 px-3 py-2">
					{/* File Type */}
					<div className="space-y-1.5">
						<span className="font-medium text-[11px] text-muted-foreground uppercase tracking-wide">
							File Type
						</span>
						<div className="flex flex-wrap gap-1.5">
							{[
								{ value: null, label: "All" },
								{ value: "files_only" as const, label: "Files" },
								{ value: "directories_only" as const, label: "Folders" },
								{ value: "media_only" as const, label: "Media" },
							].map((opt) => (
								<Button
									key={opt.label}
									variant={
										filters.file_type === opt.value ? "default" : "outline"
									}
									size="sm"
									className="h-6 px-2 text-[11px]"
									onClick={() =>
										setFilters((f) => ({ ...f, file_type: opt.value }))
									}
								>
									{opt.label}
								</Button>
							))}
						</div>
					</div>

					{/* Size Filter */}
					<div className="space-y-1.5">
						<span className="font-medium text-[11px] text-muted-foreground uppercase tracking-wide">
							Size
						</span>
						<div className="flex flex-wrap gap-1.5">
							{SIZE_PRESETS.map((preset) => (
								<Button
									key={preset.label}
									variant={
										filters.size_min === preset.min &&
										filters.size_max === preset.max
											? "default"
											: "outline"
									}
									size="sm"
									className="h-6 px-2 text-[11px]"
									onClick={() => setSizePreset(preset.min, preset.max)}
								>
									{preset.label}
								</Button>
							))}
						</div>
					</div>

					{/* Date Filter */}
					<div className="space-y-1.5">
						<span className="font-medium text-[11px] text-muted-foreground uppercase tracking-wide">
							Modified Date
						</span>
						<div className="flex items-center gap-2">
							<Input
								type="date"
								value={filters.date_min ?? ""}
								onChange={(e) =>
									setFilters((f) => ({
										...f,
										date_min: e.target.value || null,
									}))
								}
								className="h-7 w-32 text-xs"
							/>
							<span className="text-muted-foreground text-xs">to</span>
							<Input
								type="date"
								value={filters.date_max ?? ""}
								onChange={(e) =>
									setFilters((f) => ({
										...f,
										date_max: e.target.value || null,
									}))
								}
								className="h-7 w-32 text-xs"
							/>
						</div>
					</div>

					{/* Extension Filter */}
					<div className="space-y-1.5">
						<span className="font-medium text-[11px] text-muted-foreground uppercase tracking-wide">
							Extension
						</span>
						<div className="flex flex-wrap gap-1">
							{(availableExtensions.length > 0
								? availableExtensions.slice(0, 12)
								: COMMON_EXTENSIONS
							).map((ext) => (
								<button
									key={ext}
									type="button"
									onClick={() => toggleExtension(ext)}
									className={cn(
										"rounded border px-1.5 py-0.5 text-[10px] transition-colors",
										filters.extensions.includes(ext)
											? "border-primary bg-primary text-primary-foreground"
											: "border-border bg-background hover:bg-accent",
									)}
								>
									.{ext}
								</button>
							))}
						</div>
					</div>

					{/* Advanced Media Filters */}
					<div className="space-y-1.5">
						<button
							type="button"
							onClick={() => setShowAdvanced(!showAdvanced)}
							className="flex items-center gap-1 font-medium text-[11px] text-muted-foreground uppercase tracking-wide hover:text-foreground"
						>
							Media Filters
							<span className="text-[10px]">{showAdvanced ? "▼" : "▶"}</span>
						</button>

						{showAdvanced && (
							<div className="space-y-2 rounded border border-border/50 bg-background/50 p-2">
								{/* Resolution Presets */}
								<div className="space-y-1">
									<span className="text-[10px] text-muted-foreground">
										Resolution
									</span>
									<div className="flex flex-wrap gap-1">
										{RESOLUTION_PRESETS.map((preset) => (
											<Button
												key={preset.label}
												variant={
													filters.media_filters?.min_width === preset.minWidth
														? "default"
														: "outline"
												}
												size="sm"
												className="h-5 px-1.5 text-[10px]"
												onClick={() =>
													setResolutionPreset(preset.minWidth, preset.minHeight)
												}
											>
												{preset.label}
											</Button>
										))}
									</div>
								</div>

								{/* Video Codecs */}
								<div className="space-y-1">
									<span className="text-[10px] text-muted-foreground">
										Video Codec
									</span>
									<div className="flex flex-wrap gap-1">
										{COMMON_VIDEO_CODECS.map((codec) => (
											<button
												key={codec}
												type="button"
												onClick={() => toggleVideoCodec(codec)}
												className={cn(
													"rounded border px-1.5 py-0.5 text-[10px] transition-colors",
													filters.media_filters?.video_codecs?.includes(codec)
														? "border-primary bg-primary text-primary-foreground"
														: "border-border bg-background hover:bg-accent",
												)}
											>
												{codec.toUpperCase()}
											</button>
										))}
									</div>
								</div>

								{/* Audio Codecs */}
								<div className="space-y-1">
									<span className="text-[10px] text-muted-foreground">
										Audio Codec
									</span>
									<div className="flex flex-wrap gap-1">
										{COMMON_AUDIO_CODECS.map((codec) => (
											<button
												key={codec}
												type="button"
												onClick={() => toggleAudioCodec(codec)}
												className={cn(
													"rounded border px-1.5 py-0.5 text-[10px] transition-colors",
													filters.media_filters?.audio_codecs?.includes(codec)
														? "border-primary bg-primary text-primary-foreground"
														: "border-border bg-background hover:bg-accent",
												)}
											>
												{codec.toUpperCase()}
											</button>
										))}
									</div>
								</div>
							</div>
						)}
					</div>

					{/* Apply Button */}
					<div className="flex items-center gap-2 pt-1">
						<Button
							onClick={applyFilters}
							disabled={loading || !isFiltersActive}
							size="sm"
							className="h-7"
						>
							{loading ? "Applying..." : "Apply Filters"}
						</Button>
						{isFiltersActive && (
							<Button
								variant="outline"
								onClick={clearFilters}
								size="sm"
								className="h-7"
							>
								Clear
							</Button>
						)}
					</div>
				</div>
			)}
		</div>
	);
}
