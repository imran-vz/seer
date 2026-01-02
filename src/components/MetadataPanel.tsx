import { invoke } from "@tauri-apps/api/core";
import { Fragment, useEffect, useState } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";

interface FileMetadata {
	path: string;
	name: string;
	size: number;
	modified: string | null;
	created: string | null;
	is_media: boolean;
	extension: string | null;
	ffprobe_data: string | null;
}

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
		<div className="mb-4 p-3 bg-card rounded-lg border border-border">
			<h3 className="text-xs font-semibold uppercase tracking-wider text-primary mb-3">
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
			<span className="text-muted-foreground whitespace-nowrap">{label}</span>
			<span className="text-foreground wrap-break-word min-w-0">{value}</span>
		</>
	);
}

export function MetadataPanel({ filePath }: MetadataPanelProps) {
	const [metadata, setMetadata] = useState<FileMetadata | null>(null);
	const [ffprobe, setFfprobe] = useState<FFProbeData | null>(null);
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		if (!filePath) {
			setMetadata(null);
			setFfprobe(null);
			return;
		}

		const loadMetadata = async () => {
			setLoading(true);
			setError(null);
			try {
				const data = await invoke<FileMetadata>("get_file_metadata", {
					path: filePath,
				});
				setMetadata(data);
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
			} finally {
				setLoading(false);
			}
		};

		loadMetadata();
	}, [filePath]);

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
			<div className="flex items-center justify-center h-full text-muted-foreground">
				<p>Select a file to view metadata</p>
			</div>
		);
	}

	if (loading) {
		return (
			<div className="flex items-center justify-center h-full text-muted-foreground">
				<p>Loading metadata...</p>
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex items-center justify-center h-full text-destructive">
				<p>Error: {error}</p>
			</div>
		);
	}

	if (!metadata) return null;

	return (
		<ScrollArea className="h-full">
			<div className="p-4">
				<h2 className="text-lg font-semibold mb-4 break-all">
					{metadata.name}
				</h2>

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
