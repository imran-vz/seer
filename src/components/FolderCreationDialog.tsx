import { invoke } from "@tauri-apps/api/core";
import { FolderPlus } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

interface FolderCreationDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	paths: string[];
	currentPath: string;
	onSuccess: () => void;
}

type FolderCreationMode =
	| { type: "per_file" }
	| { type: "grouped"; criteria: GroupCriteria }
	| { type: "single"; name: string };

type GroupCriteria =
	| { type: "extension" }
	| { type: "date_modified"; granularity: string }
	| { type: "media_type" }
	| { type: "resolution" }
	| { type: "codec" };

interface FolderCreationResult {
	success: number;
	failed: number;
	folders_created: string[];
	errors: string[];
}

export function FolderCreationDialog({
	open,
	onOpenChange,
	paths,
	currentPath,
	onSuccess,
}: FolderCreationDialogProps) {
	const [modeType, setModeType] = useState<"per_file" | "grouped" | "single">(
		"per_file",
	);
	const [groupCriteria, setGroupCriteria] = useState<
		"extension" | "date_modified" | "media_type" | "resolution" | "codec"
	>("extension");
	const [dateGranularity, setDateGranularity] = useState<
		"day" | "month" | "year"
	>("day");
	const [folderName, setFolderName] = useState("");
	const [executing, setExecuting] = useState(false);
	const [error, setError] = useState<string | null>(null);

	const handleCreate = async () => {
		setExecuting(true);
		setError(null);

		try {
			// Build the mode object
			let mode: FolderCreationMode;

			if (modeType === "per_file") {
				mode = { type: "per_file" };
			} else if (modeType === "grouped") {
				let criteria: GroupCriteria;
				if (groupCriteria === "extension") {
					criteria = { type: "extension" };
				} else if (groupCriteria === "date_modified") {
					criteria = { type: "date_modified", granularity: dateGranularity };
				} else if (groupCriteria === "media_type") {
					criteria = { type: "media_type" };
				} else if (groupCriteria === "resolution") {
					criteria = { type: "resolution" };
				} else {
					criteria = { type: "codec" };
				}
				mode = { type: "grouped", criteria };
			} else {
				if (!folderName.trim()) {
					setError("Please enter a folder name");
					setExecuting(false);
					return;
				}
				mode = { type: "single", name: folderName.trim() };
			}

			const result = await invoke<FolderCreationResult>(
				"create_folders_from_selection",
				{
					paths,
					mode,
					parentDir: currentPath,
				},
			);

			if (result.failed > 0) {
				toast.error(
					`Created ${result.success} folders, ${result.failed} failed: ${result.errors.join(", ")}`,
				);
			} else {
				toast.success(
					`Successfully created ${result.folders_created.length} folders and moved ${result.success} files`,
				);
			}

			onSuccess();
			onOpenChange(false);
		} catch (e) {
			console.error("Folder creation error:", e);
			setError(String(e));
			toast.error(String(e));
		} finally {
			setExecuting(false);
		}
	};

	const getModeDescription = () => {
		if (modeType === "per_file") {
			return "Create a folder for each file and move the file into it";
		}
		if (modeType === "grouped") {
			if (groupCriteria === "extension") {
				return "Group files by extension (mp4, mkv, etc.)";
			}
			if (groupCriteria === "date_modified") {
				return `Group files by modification date (${dateGranularity})`;
			}
			if (groupCriteria === "media_type") {
				return "Group files by type (video, audio, other)";
			}
			if (groupCriteria === "resolution") {
				return "Group media files by resolution (1080p, 720p, etc.) - requires ffprobe";
			}
			return "Group media files by codec - requires ffprobe";
		}
		return "Move all files into a single new folder";
	};

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="max-w-lg">
				<DialogHeader>
					<DialogTitle>Create Folders from Selection</DialogTitle>
					<DialogDescription>
						{paths.length} files selected in {currentPath}
					</DialogDescription>
				</DialogHeader>

				<div className="space-y-4 py-4">
					{/* Mode Selection */}
					<div className="space-y-2">
						<label className="font-medium text-sm">Mode</label>
						<div className="space-y-2">
							<div className="flex items-center space-x-2">
								<input
									type="radio"
									id="per_file"
									name="mode"
									checked={modeType === "per_file"}
									onChange={() => setModeType("per_file")}
									className="cursor-pointer"
								/>
								<label
									htmlFor="per_file"
									className="cursor-pointer font-medium text-sm"
								>
									Folder per file
								</label>
							</div>
							<div className="flex items-center space-x-2">
								<input
									type="radio"
									id="grouped"
									name="mode"
									checked={modeType === "grouped"}
									onChange={() => setModeType("grouped")}
									className="cursor-pointer"
								/>
								<label
									htmlFor="grouped"
									className="cursor-pointer font-medium text-sm"
								>
									Group by criteria
								</label>
							</div>
							<div className="flex items-center space-x-2">
								<input
									type="radio"
									id="single"
									name="mode"
									checked={modeType === "single"}
									onChange={() => setModeType("single")}
									className="cursor-pointer"
								/>
								<label
									htmlFor="single"
									className="cursor-pointer font-medium text-sm"
								>
									Single folder
								</label>
							</div>
						</div>
					</div>

					{/* Group Criteria (shown when grouped) */}
					{modeType === "grouped" && (
						<div className="space-y-2">
							<label htmlFor="group-criteria" className="font-medium text-sm">
								Group by
							</label>
							<select
								id="group-criteria"
								value={groupCriteria}
								onChange={(e) =>
									setGroupCriteria(
										e.target.value as
											| "extension"
											| "date_modified"
											| "media_type"
											| "resolution"
											| "codec",
									)
								}
								className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
							>
								<option value="extension">Extension</option>
								<option value="date_modified">Date Modified</option>
								<option value="media_type">Media Type</option>
								<option value="resolution">Resolution</option>
								<option value="codec">Codec</option>
							</select>

							{/* Date Granularity (shown when date_modified) */}
							{groupCriteria === "date_modified" && (
								<div className="mt-2">
									<label
										htmlFor="date-granularity"
										className="font-medium text-sm"
									>
										Granularity
									</label>
									<select
										id="date-granularity"
										value={dateGranularity}
										onChange={(e) =>
											setDateGranularity(
												e.target.value as "day" | "month" | "year",
											)
										}
										className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:ring-ring focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2"
									>
										<option value="day">Day (YYYY-MM-DD)</option>
										<option value="month">Month (YYYY-MM)</option>
										<option value="year">Year (YYYY)</option>
									</select>
								</div>
							)}
						</div>
					)}

					{/* Folder Name (shown when single) */}
					{modeType === "single" && (
						<div className="space-y-2">
							<label htmlFor="folder-name" className="font-medium text-sm">
								Folder name
							</label>
							<Input
								id="folder-name"
								value={folderName}
								onChange={(e) => setFolderName(e.target.value)}
								placeholder="Enter folder name"
							/>
						</div>
					)}

					{/* Description */}
					<div className="rounded-md bg-muted p-3 text-muted-foreground text-xs">
						{getModeDescription()}
					</div>

					{/* Error */}
					{error && <p className="text-destructive text-sm">{error}</p>}
				</div>

				<DialogFooter>
					<Button variant="outline" onClick={() => onOpenChange(false)}>
						Cancel
					</Button>
					<Button onClick={handleCreate} disabled={executing}>
						<FolderPlus className="mr-2 size-4" />
						{executing ? "Creating..." : "Create"}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
