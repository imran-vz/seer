import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useMetadataStore } from "@/stores/metadataStore";
import type {
	MetadataEntry,
	MetadataOperation,
	MetadataScope,
} from "@/types/metadata";

interface MetadataPanelProps {
	filePath: string | null;
}

const QUICK_FIELDS: { label: string; key: string; scope: MetadataScope }[] = [
	{ label: "Title", key: "title", scope: "format" },
	{ label: "Description", key: "description", scope: "format" },
	{ label: "Artist", key: "artist", scope: "format" },
	{ label: "Album", key: "album", scope: "format" },
	{ label: "Genre", key: "genre", scope: "format" },
	{ label: "Year", key: "date", scope: "format" },
	{ label: "Language", key: "language", scope: "stream" },
	{ label: "Duration Tag", key: "duration", scope: "format" },
];

function TagSection({
	title,
	tags,
	onPrefill,
	onDelete,
}: {
	title: string;
	tags: MetadataEntry[];
	onPrefill: (entry: MetadataEntry) => void;
	onDelete: (entry: MetadataEntry) => void;
}) {
	if (tags.length === 0) return null;

	return (
		<div className="mb-4 rounded-lg border border-border bg-card p-3">
			<div className="mb-2 flex items-center justify-between text-muted-foreground text-xs uppercase tracking-wider">
				<span>{title}</span>
				<span>{tags.length} tags</span>
			</div>
			<div className="space-y-2 text-sm">
				{tags.map((tag, idx) => (
					<div
						key={`${tag.key}-${tag.streamIndex ?? "format"}-${idx}`}
						className="grid grid-cols-[1fr_auto] items-start gap-2 rounded border border-border/60 bg-muted/40 p-2"
					>
						<div className="space-y-1">
							<div className="flex flex-wrap items-center gap-2 text-muted-foreground text-xs">
								<Badge variant="outline" className="border-border/70">
									{tag.scope.toUpperCase()}
								</Badge>
								<Badge variant="outline" className="border-border/70">
									{tag.origin}
								</Badge>
								{tag.streamIndex !== undefined && (
									<span className="rounded bg-border px-2 py-0.5 text-[10px]">
										Stream {tag.streamIndex}
									</span>
								)}
							</div>
							<div className="font-semibold text-foreground">{tag.key}</div>
							<div className="wrap-break-word whitespace-pre-wrap text-foreground">
								{tag.value || "-"}
							</div>
						</div>
						<div className="flex flex-col gap-2">
							<Button
								variant="secondary"
								size="sm"
								onClick={() => onPrefill(tag)}
							>
								Edit
							</Button>
							<Button variant="ghost" size="sm" onClick={() => onDelete(tag)}>
								Delete
							</Button>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

export function MetadataPanel({ filePath }: MetadataPanelProps) {
	const {
		snapshot,
		loading,
		saving,
		error,
		pendingOperations,
		toolAvailability,
		load,
		reset,
		stageOperation,
		removeOperation,
		clearOperations,
		applyOperations,
	} = useMetadataStore();

	const [keyInput, setKeyInput] = useState("");
	const [valueInput, setValueInput] = useState("");
	const [scope, setScope] = useState<MetadataScope>("format");
	const [streamIndex, setStreamIndex] = useState<string>("");
	const keyFieldId = "metadata-key";
	const valueFieldId = "metadata-value";
	const scopeFieldId = "metadata-scope";
	const streamIndexId = "metadata-stream-index";

	useEffect(() => {
		if (filePath) {
			void load(filePath);
		} else {
			reset();
		}
	}, [filePath, load, reset]);

	const formatSize = (bytes: number): string => {
		const units = ["B", "KB", "MB", "GB", "TB"];
		const i = Math.min(
			units.length - 1,
			Math.floor(Math.log(bytes || 1) / Math.log(1024)),
		);
		return `${(bytes / 1024 ** i).toFixed(2)} ${units[i]}`;
	};

	const allTags = useMemo(() => {
		return {
			format: snapshot?.formatTags ?? [],
			streams: snapshot?.streamTags ?? [],
			file: snapshot?.fileTags ?? [],
		};
	}, [snapshot]);

	const resetForm = () => {
		setKeyInput("");
		setValueInput("");
		setScope("format");
		setStreamIndex("");
	};

	const pushOperation = (action: MetadataOperation["action"]) => {
		if (!keyInput.trim()) {
			toast.error("Metadata key is required");
			return;
		}

		let streamIdx: number | undefined;
		if (scope === "stream") {
			if (streamIndex === "") {
				toast.error("Stream index is required for stream scope");
				return;
			}
			const parsed = Number(streamIndex);
			if (Number.isNaN(parsed)) {
				toast.error("Stream index must be a number");
				return;
			}
			streamIdx = parsed;
		}

		const operation: MetadataOperation = {
			action,
			key: keyInput.trim(),
			value: action === "delete" ? undefined : valueInput,
			scope,
			streamIndex: streamIdx,
		};

		stageOperation(operation);
		resetForm();
	};

	const handleSave = async () => {
		const result = await applyOperations();
		if (result?.success) {
			toast.success("Metadata updated");
		} else if (result === null && error) {
			toast.error(error);
		}
	};

	const handlePrefill = (entry: MetadataEntry) => {
		setKeyInput(entry.key);
		setValueInput(entry.value);
		setScope(entry.scope);
		setStreamIndex(entry.streamIndex?.toString() ?? "");
	};

	const handleDeleteStage = (entry: MetadataEntry) => {
		stageOperation({
			action: "delete",
			key: entry.key,
			scope: entry.scope,
			streamIndex: entry.streamIndex,
		});
	};

	const handleClearAll = () => {
		if (!snapshot) return;
		const ops: MetadataOperation[] = [];
		if (toolAvailability?.ffmpeg) {
			ops.push({ action: "delete", key: "*", scope: "format" });
			const streamIndices = Array.from(
				new Set(
					snapshot.streamSummaries
						.filter((s) => s.index >= 0)
						.map((s) => s.index),
				),
			);
			for (const idx of streamIndices) {
				ops.push({
					action: "delete",
					key: "*",
					scope: "stream",
					streamIndex: idx,
				});
			}
		}
		if (toolAvailability?.exiftool) {
			ops.push({ action: "delete", key: "*", scope: "file" });
		}

		if (ops.length === 0) {
			toast.error("No tools available to clear metadata");
			return;
		}

		for (const op of ops) stageOperation(op);
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

	if (!snapshot) return null;

	return (
		<ScrollArea className="h-full">
			<div className="flex flex-col gap-4 p-4">
				<div className="flex flex-wrap items-start justify-between gap-3">
					<div>
						<h2 className="break-all font-semibold text-lg">
							{snapshot.fileName}
						</h2>
						<p className="text-muted-foreground text-xs">{snapshot.path}</p>
					</div>
					<div className="flex flex-wrap items-center gap-2 text-muted-foreground text-xs">
						<Badge variant="outline">
							{toolAvailability?.ffmpeg ? "FFmpeg" : "FFmpeg missing"}
						</Badge>
						<Badge variant="outline">
							{toolAvailability?.ffprobe ? "FFprobe" : "FFprobe missing"}
						</Badge>
						<Badge variant="outline">
							{toolAvailability?.exiftool ? "ExifTool" : "ExifTool missing"}
						</Badge>
						<Button
							variant="secondary"
							size="sm"
							onClick={() => load(snapshot.path)}
						>
							Refresh
						</Button>
					</div>
				</div>

				<div className="grid grid-cols-1 gap-3 md:grid-cols-2">
					<div className="rounded-lg border border-border bg-card p-3 text-sm">
						<div className="mb-2 text-muted-foreground text-xs uppercase tracking-wider">
							File Info
						</div>
						<div className="space-y-1">
							<div className="flex justify-between text-sm">
								<span className="text-muted-foreground">Size</span>
								<span className="text-foreground">
									{formatSize(snapshot.size)}
								</span>
							</div>
							<div className="flex justify-between text-sm">
								<span className="text-muted-foreground">Type</span>
								<span className="text-foreground">
									{snapshot.extension?.toUpperCase() || "Unknown"}
								</span>
							</div>
							<div className="flex justify-between text-sm">
								<span className="text-muted-foreground">Modified</span>
								<span className="text-foreground">
									{snapshot.modified || "-"}
								</span>
							</div>
							<div className="flex justify-between text-sm">
								<span className="text-muted-foreground">Created</span>
								<span className="text-foreground">
									{snapshot.created || "-"}
								</span>
							</div>
						</div>
					</div>

					<div className="rounded-lg border border-border bg-card p-3">
						<div className="mb-2 text-muted-foreground text-xs uppercase tracking-wider">
							Add or Edit Metadata
						</div>
						<div className="grid grid-cols-1 gap-2 md:grid-cols-2">
							<div className="space-y-1">
								<label
									htmlFor={keyFieldId}
									className="text-muted-foreground text-xs"
								>
									Key
								</label>
								<Input
									id={keyFieldId}
									value={keyInput}
									onChange={(e) => setKeyInput(e.target.value)}
									placeholder="e.g. title"
								/>
							</div>
							<div className="space-y-1">
								<label
									htmlFor={valueFieldId}
									className="text-muted-foreground text-xs"
								>
									Value
								</label>
								<textarea
									id={valueFieldId}
									value={valueInput}
									onChange={(e) => setValueInput(e.target.value)}
									className="min-h-[42px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm shadow-sm outline-none focus:ring-2 focus:ring-ring"
									placeholder="Value (leave empty to delete)"
								/>
							</div>
							<div className="space-y-1">
								<label
									htmlFor={scopeFieldId}
									className="text-muted-foreground text-xs"
								>
									Scope
								</label>
								<select
									id={scopeFieldId}
									value={scope}
									onChange={(e) => setScope(e.target.value as MetadataScope)}
									className="h-10 w-full rounded-md border border-input bg-background px-3 text-sm"
								>
									<option value="format">Format</option>
									<option value="stream">Stream</option>
									<option value="file">File (Exif)</option>
								</select>
							</div>
							<div className="space-y-1">
								<label
									htmlFor={streamIndexId}
									className="text-muted-foreground text-xs"
								>
									Stream Index (for stream scope)
								</label>
								<Input
									id={streamIndexId}
									type="number"
									value={streamIndex}
									onChange={(e) => setStreamIndex(e.target.value)}
									disabled={scope !== "stream"}
									placeholder="0"
								/>
							</div>
						</div>
						<div className="flex flex-wrap items-center gap-2">
							<Button onClick={() => pushOperation("set")}>Stage Set</Button>
							<Button
								variant="secondary"
								onClick={() => pushOperation("delete")}
							>
								Stage Delete
							</Button>
							<Button variant="ghost" onClick={resetForm}>
								Clear Form
							</Button>
							<Button variant="ghost" onClick={handleClearAll}>
								Wipe All Tags
							</Button>
						</div>
						<div className="flex flex-wrap gap-2 text-xs">
							{QUICK_FIELDS.map((field) => (
								<Button
									key={field.key}
									variant="outline"
									size="sm"
									onClick={() => {
										setKeyInput(field.key);
										setScope(field.scope);
									}}
								>
									{field.label}
								</Button>
							))}
						</div>
					</div>
				</div>

				<div className="rounded-lg border border-border bg-card p-3">
					<div className="mb-2 flex items-center justify-between text-muted-foreground text-xs uppercase tracking-wider">
						<span>Pending Changes</span>
						{pendingOperations.length > 0 && (
							<Button variant="ghost" size="sm" onClick={clearOperations}>
								Clear
							</Button>
						)}
					</div>
					{pendingOperations.length === 0 ? (
						<p className="text-muted-foreground text-sm">
							No pending operations
						</p>
					) : (
						<div className="space-y-2">
							{pendingOperations.map((op, idx) => (
								<div
									key={`${op.key}-${idx}`}
									className="flex items-start justify-between rounded border border-border/60 bg-muted/40 p-2 text-sm"
								>
									<div className="space-y-1">
										<div className="flex flex-wrap items-center gap-2 text-muted-foreground text-xs">
											<Badge variant="outline">{op.action}</Badge>
											<Badge variant="outline">{op.scope}</Badge>
											{op.streamIndex !== undefined && (
												<span className="rounded bg-border px-2 py-0.5 text-[10px]">
													Stream {op.streamIndex}
												</span>
											)}
										</div>
										<div className="font-semibold text-foreground">
											{op.key}
										</div>
										{op.value !== undefined && (
											<div className="wrap-break-word whitespace-pre-wrap">
												{op.value}
											</div>
										)}
									</div>
									<Button
										variant="ghost"
										size="sm"
										onClick={() => removeOperation(idx)}
									>
										Remove
									</Button>
								</div>
							))}
						</div>
					)}
					<div className="mt-2 flex justify-end">
						<Button
							disabled={pendingOperations.length === 0 || saving}
							onClick={handleSave}
						>
							{saving ? "Saving..." : "Apply Changes"}
						</Button>
					</div>
				</div>

				<TagSection
					title="Format Tags"
					tags={allTags.format}
					onPrefill={handlePrefill}
					onDelete={handleDeleteStage}
				/>
				<TagSection
					title="Stream Tags"
					tags={allTags.streams}
					onPrefill={handlePrefill}
					onDelete={handleDeleteStage}
				/>
				<TagSection
					title="File Tags (EXIF/XMP)"
					tags={allTags.file}
					onPrefill={handlePrefill}
					onDelete={handleDeleteStage}
				/>
			</div>
		</ScrollArea>
	);
}
