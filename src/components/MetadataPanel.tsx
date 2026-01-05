import { Fragment, useCallback, useEffect, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	type CachedFileMetadata,
	getFileMetadataCached,
} from "@/lib/fileMetadataCache";

interface FFProbeFormat {
	filename?: string;
	format_name?: string;
	format_long_name?: string;
	duration?: string;
	size?: string;
	bit_rate?: string;
	tags?: Record<string, string>;
}

interface FFProbeStream {
	codec_type?: string;
	codec_name?: string;
	codec_long_name?: string;
	width?: number;
	height?: number;
	sample_rate?: string;
	channels?: number;
	bit_rate?: string;
	duration?: string;
	tags?: Record<string, string>;
}

interface FFProbeData {
	format?: FFProbeFormat;
	streams?: FFProbeStream[];
}

interface MetadataPanelProps {
	filePath: string | null;
}

function MetadataSection({
	title,
	children,
}: {
	title: string;
	children: React.ReactNode;
}) {
	return (
		<div className="mb-4 rounded-lg border border-border bg-card p-3">
			<h3 className="mb-3 font-semibold text-primary text-xs uppercase tracking-wider">
				{title}
			</h3>
			<div className="grid grid-cols-[minmax(100px,auto)_1fr] gap-x-4 gap-y-2 text-sm">
				{children}
			</div>
		</div>
	);
}

function MetadataRow({
	label,
	value,
}: {
	label: string;
	value: string | number;
}) {
	return (
		<>
			<span className="whitespace-nowrap text-muted-foreground">{label}</span>
			<span className="wrap-break-word min-w-0 text-foreground">{value}</span>
		</>
	);
}

export function MetadataPanel({ filePath }: MetadataPanelProps) {
	const [metadata, setMetadata] = useState<CachedFileMetadata | null>(null);
	const [ffprobe, setFfprobe] = useState<FFProbeData | null>(null);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [fromCache, setFromCache] = useState(false);

	const loadMetadata = useCallback(
		async (forceRefresh = false) => {
			if (!filePath) {
				setMetadata(null);
				setFfprobe(null);
				setFromCache(false);
				return;
			}

			setLoading(true);
			setError(null);

			try {
				// Use cached metadata with hash validation
				// The cache will automatically validate that the file hasn't changed
				// by comparing the stored hash with the current file's hash
				const data = await getFileMetadataCached(filePath, {
					forceRefresh,
				});

				setMetadata(data);
				setFromCache(data.from_cache);

				if (data.ffprobe_data) {
					try {
						setFfprobe(JSON.parse(data.ffprobe_data));
					} catch {
						setFfprobe(null);
					}
				} else {
					setFfprobe(null);
				}
			} catch (e) {
				setError(String(e));
				setMetadata(null);
				setFfprobe(null);
				setFromCache(false);
			} finally {
				setLoading(false);
			}
		},
		[filePath],
	);

	useEffect(() => {
		loadMetadata();
	}, [loadMetadata]);

	const formatSize = (bytes: number): string => {
		const units = ["B", "KB", "MB", "GB"];
		const i = Math.floor(Math.log(bytes) / Math.log(1024));
		return `${(bytes / 1024 ** i).toFixed(2)} ${units[i]}`;
	};

	const formatDuration = (seconds: string): string => {
		const s = parseFloat(seconds);
		const h = Math.floor(s / 3600);
		const m = Math.floor((s % 3600) / 60);
		const sec = Math.floor(s % 60);
		if (h > 0)
			return `${h}:${m.toString().padStart(2, "0")}:${sec.toString().padStart(2, "0")}`;
		return `${m}:${sec.toString().padStart(2, "0")}`;
	};

	if (!filePath) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<p>Select a file to view metadata</p>
			</div>
		);
	}

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<p>Loading metadata...</p>
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex h-full items-center justify-center text-destructive">
				<p>Error: {error}</p>
			</div>
		);
	}

	if (!metadata) return null;

	return (
		<ScrollArea className="h-full">
			<div className="p-4">
				<div className="mb-4 flex items-start justify-between gap-2">
					<h2 className="break-all font-semibold text-lg">{metadata.name}</h2>
					{fromCache && (
						<button
							type="button"
							onClick={() => loadMetadata(true)}
							className="shrink-0 rounded px-2 py-1 text-muted-foreground text-xs hover:bg-muted hover:text-foreground"
							title="Refresh metadata from disk"
						>
							↻ Refresh
						</button>
					)}
				</div>

				{fromCache && metadata.cached_at && (
					<div className="mb-4 rounded border border-border bg-muted/50 px-3 py-2 text-muted-foreground text-xs">
						Cached{" "}
						{new Date(metadata.cached_at).toLocaleString(undefined, {
							dateStyle: "short",
							timeStyle: "short",
						})}
					</div>
				)}

				<MetadataSection title="File Info">
					<MetadataRow label="Size" value={formatSize(metadata.size)} />
					<MetadataRow
						label="Type"
						value={metadata.extension?.toUpperCase() || "Unknown"}
					/>
					<MetadataRow label="Created" value={metadata.created || "-"} />
					<MetadataRow label="Modified" value={metadata.modified || "-"} />
				</MetadataSection>

				{ffprobe?.format && (
					<MetadataSection title="Format">
						<MetadataRow
							label="Container"
							value={
								ffprobe.format.format_long_name ||
								ffprobe.format.format_name ||
								"-"
							}
						/>
						{ffprobe.format.duration && (
							<MetadataRow
								label="Duration"
								value={formatDuration(ffprobe.format.duration)}
							/>
						)}
						{ffprobe.format.bit_rate && (
							<MetadataRow
								label="Bitrate"
								value={`${Math.round(parseInt(ffprobe.format.bit_rate, 10) / 1000)} kbps`}
							/>
						)}
					</MetadataSection>
				)}

				{ffprobe?.streams?.map((stream, i) => (
					<MetadataSection
						// biome-ignore lint/suspicious/noArrayIndexKey: ffprobe streams are text with no reliable IDs
						key={i}
						title={`${stream.codec_type === "video" ? "Video" : stream.codec_type === "audio" ? "Audio" : "Stream"} #${i + 1}`}
					>
						<MetadataRow
							label="Codec"
							value={stream.codec_long_name || stream.codec_name || "-"}
						/>
						{stream.width && stream.height && (
							<MetadataRow
								label="Resolution"
								value={`${stream.width}x${stream.height}`}
							/>
						)}
						{stream.sample_rate && (
							<MetadataRow
								label="Sample Rate"
								value={`${parseInt(stream.sample_rate, 10) / 1000} kHz`}
							/>
						)}
						{stream.channels && (
							<MetadataRow label="Channels" value={stream.channels} />
						)}
						{stream.bit_rate && (
							<MetadataRow
								label="Bitrate"
								value={`${Math.round(parseInt(stream.bit_rate, 10) / 1000)} kbps`}
							/>
						)}
					</MetadataSection>
				))}

				{ffprobe?.format?.tags &&
					Object.keys(ffprobe.format.tags).length > 0 && (
						<MetadataSection title="Tags">
							{Object.entries(ffprobe.format.tags).map(([key, value]) => (
								<Fragment key={key}>
									<MetadataRow label={key} value={value} />
								</Fragment>
							))}
						</MetadataSection>
					)}
			</div>
		</ScrollArea>
	);
}
