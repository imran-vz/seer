import { AlertTriangle, FileVideo } from "lucide-react";
import { useEffect, useMemo } from "react";
import { toast } from "sonner";

import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { isImageFile, isVideoOrAudioFile } from "@/lib/fileUtils";
import { useMetadataStore } from "@/stores/metadataStore";
import type { MetadataEntry, MetadataScope } from "@/types/metadata";

import { AddTagDialog } from "./metadata/AddTagDialog";
import { MetadataHeader } from "./metadata/MetadataHeader";
import { PendingChangesBar } from "./metadata/PendingChangesBar";
import { TagGroup } from "./metadata/TagGroup";
import { type DisplayTag, sortTags } from "./metadata/utils";

interface MetadataPanelProps {
	filePath: string | null;
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

	useEffect(() => {
		if (filePath) {
			void load(filePath);
		} else {
			reset();
		}
	}, [filePath, load, reset]);

	const mergedTags = useMemo(() => {
		if (!snapshot) return { format: [], streams: [], file: [] };

		// Helper to find pending op for a tag
		const findOp = (
			key: string,
			scope: MetadataScope,
			streamIndex?: number,
		) => {
			// Search in reverse to get latest operation
			for (let i = pendingOperations.length - 1; i >= 0; i--) {
				const op = pendingOperations[i];
				if (
					op.key === key &&
					op.scope === scope &&
					op.streamIndex === streamIndex
				) {
					return { op, index: i };
				}
			}
			return null;
		};

		// Process tags with pending operations
		const processTags = (
			originalTags: MetadataEntry[],
			scope: MetadataScope,
		) => {
			const processed: DisplayTag[] = originalTags.map((tag) => {
				const pending = findOp(tag.key, scope, tag.streamIndex);
				return { ...tag, pending: pending || undefined };
			});

			// Add "new" tags that are in pending ops but not in original
			pendingOperations.forEach((op, idx) => {
				if (op.scope !== scope) return;

				// Calculate if this op effectively deletes the tag it created (or previous ops created)
				// But simpler: just check if the LATEST op for this key is a delete, if so, don't show as a "new" tag
				const latestOp = findOp(op.key, op.scope, op.streamIndex);
				if (latestOp && latestOp.op.action === "delete") {
					// If it's a delete op, we only care if it was in the original list (handled above)
					// If it wasn't in original list, it shouldn't be added here
					return;
				}

				const exists = processed.some(
					(t) => t.key === op.key && t.streamIndex === op.streamIndex,
				);

				if (!exists) {
					processed.push({
						key: op.key,
						value: op.value || "",
						scope: op.scope,
						streamIndex: op.streamIndex,
						origin: "pending",
						editable: true,
						pending: { op, index: idx },
					});
				}
			});

			return sortTags(processed);
		};

		return {
			format: processTags(snapshot.formatTags, "format"),
			streams: processTags(snapshot.streamTags, "stream"),
			file: processTags(snapshot.fileTags, "file"),
		};
	}, [snapshot, pendingOperations]);

	const handleStageSet = (
		key: string,
		value: string,
		scope: MetadataScope,
		streamIndex?: number,
	) => {
		stageOperation({
			action: "set",
			key,
			value,
			scope,
			streamIndex,
		});
	};

	const handleStageDelete = (
		key: string,
		scope: MetadataScope,
		streamIndex?: number,
	) => {
		stageOperation({
			action: "delete",
			key,
			scope,
			streamIndex,
		});
	};

	const handleApply = async () => {
		const result = await applyOperations();
		if (result?.success) {
			toast.success("Metadata updated successfully");
		} else if (error) {
			toast.error(`Failed to update metadata: ${error}`);
		}
	};

	const toolsReady = toolAvailability?.ffmpeg || toolAvailability?.exiftool;

	// Determine file type for conditional rendering
	const isImage = isImageFile(filePath);
	const isVideoAudio = isVideoOrAudioFile(filePath);

	const streamTags = useMemo(() => {
		if (isImage) return [];

		return Object.entries(
			mergedTags.streams.reduce(
				(acc, tag) => {
					const idx = tag.streamIndex ?? -1;
					if (!acc[idx]) acc[idx] = [];
					acc[idx].push(tag);
					return acc;
				},
				{} as Record<number, DisplayTag[]>,
			),
		);
	}, [mergedTags.streams, isImage]);

	if (!filePath) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<div className="text-center">
					<FileVideo className="mx-auto mb-2 h-8 w-8 opacity-50" />
					<p>Select a file to view metadata</p>
				</div>
			</div>
		);
	}

	if (loading) {
		return (
			<div className="flex h-full items-center justify-center text-muted-foreground">
				<div className="flex flex-col items-center gap-2">
					<div className="h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent" />
					<p>Loading metadata...</p>
				</div>
			</div>
		);
	}

	if (error) {
		return (
			<div className="flex h-full items-center justify-center text-destructive">
				<div className="flex flex-col items-center gap-2 px-4 text-center">
					<AlertTriangle className="h-8 w-8" />
					<p>{error}</p>
					<Button variant="outline" onClick={() => load(filePath)}>
						Retry
					</Button>
				</div>
			</div>
		);
	}

	if (!snapshot) return null;

	return (
		<div className="relative flex h-full flex-col">
			<MetadataHeader
				snapshot={snapshot}
				toolAvailability={toolAvailability}
				onRefresh={() => load(snapshot.path)}
			/>

			<ScrollArea className="min-h-[calc(100%-4rem)] flex-1">
				<div className="flex flex-col gap-6 p-4 pb-24">
					<div className="flex items-center justify-between">
						<h3 className="font-medium text-muted-foreground text-sm uppercase tracking-wider">
							Metadata
						</h3>
						<AddTagDialog disabled={!toolsReady} onAdd={handleStageSet} />
					</div>

					{/* Format Container - only for video/audio files */}
					{!isImage && (
						<TagGroup
							title="Format Container"
							tags={mergedTags.format}
							scope="format"
							disabled={!toolsReady}
							onEdit={handleStageSet}
							onDelete={handleStageDelete}
							onUndo={removeOperation}
						/>
					)}

					{/* Stream tags - only for video/audio files */}
					{!isImage &&
						streamTags.map(([index, tags]) => (
							<TagGroup
								key={`stream-${index}`}
								title={`Stream ${index}`}
								tags={tags}
								scope="stream"
								streamIndex={Number(index)}
								disabled={!toolsReady}
								onEdit={handleStageSet}
								onDelete={handleStageDelete}
								onUndo={removeOperation}
							/>
						))}

					{/* File (EXIF/XMP) - only for image files */}
					{!isVideoAudio && (
						<TagGroup
							title="File (EXIF/XMP)"
							tags={mergedTags.file}
							scope="file"
							disabled={!toolsReady}
							onEdit={handleStageSet}
							onDelete={handleStageDelete}
							onUndo={removeOperation}
						/>
					)}
				</div>
			</ScrollArea>

			<PendingChangesBar
				pendingCount={pendingOperations.length}
				saving={saving}
				onApply={handleApply}
				onClear={clearOperations}
			/>
		</div>
	);
}
