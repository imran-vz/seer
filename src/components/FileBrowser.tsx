import { invoke } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import {
	ChevronLeft,
	Copy,
	CopyIcon,
	FileText,
	Film,
	Folder,
	FolderOpen,
	FolderPlus,
	Move,
	Pencil,
	RefreshCw,
	Trash,
	Trash2,
} from "lucide-react";
import { useCallback, useState } from "react";
import { toast } from "sonner";
import { BulkActionsBar } from "@/components/BulkActionsBar";
import { BulkRenameDialog } from "@/components/BulkRenameDialog";
import { BulkStreamCleanupDialog } from "@/components/BulkStreamCleanupDialog";
import { FilterPanel } from "@/components/FilterPanel";
import { FolderCreationDialog } from "@/components/FolderCreationDialog";
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
import { Checkbox } from "@/components/ui/checkbox";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { type FileEntry, useFileBrowserStore } from "@/stores/fileBrowserStore";

type DialogType =
	| "rename"
	| "newFolder"
	| "move"
	| "copy"
	| "bulkMove"
	| "bulkCopy"
	| null;

export function FileBrowser() {
	const {
		currentPath,
		files,
		selectedPath,
		selectedPaths,
		error,
		loading,
		refresh,
		goUp,
		navigate,
		selectFile,
		renameFile,
		deleteFile,
		moveFile,
		copyFile,
		createFolder,
		revealInFolder,
		toggleSelection,
		selectAll,
		clearSelection,
		deleteSelected,
		moveSelected,
		copySelected,
		getBulkRenameablePaths,
		getMediaFilePaths,
	} = useFileBrowserStore();

	const [dialogType, setDialogType] = useState<DialogType>(null);
	const [dialogInput, setDialogInput] = useState("");
	const [targetFile, setTargetFile] = useState<FileEntry | null>(null);
	const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
	const [bulkDeleteDialogOpen, setBulkDeleteDialogOpen] = useState(false);
	const [bulkRenameDialogOpen, setBulkRenameDialogOpen] = useState(false);
	const [bulkStreamCleanupDialogOpen, setBulkStreamCleanupDialogOpen] =
		useState(false);
	const [folderCreationDialogOpen, setFolderCreationDialogOpen] =
		useState(false);
	const [operationError, setOperationError] = useState<string | null>(null);
	const [filterPanelOpen, setFilterPanelOpen] = useState(false);
	const [filteredFiles, setFilteredFiles] = useState<FileEntry[] | null>(null);

	// Handle filter results
	const handleFilterApplied = useCallback(
		(
			result: {
				files: FileEntry[];
				total_count: number;
				filtered_count: number;
			} | null,
		) => {
			if (result === null) {
				setFilteredFiles(null);
			} else {
				setFilteredFiles(result.files);
			}
		},
		[],
	);

	// Use filtered files if available, otherwise use all files
	const displayedFiles = filteredFiles ?? files;

	const formatSize = (bytes: number): string => {
		if (bytes === 0) return "-";
		const units = ["B", "KB", "MB", "GB"];
		const i = Math.floor(Math.log(bytes) / Math.log(1024));
		return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
	};

	const hasSelection = selectedPaths.size > 0;
	const allSelected = files.length > 0 && selectedPaths.size === files.length;

	const openRenameDialog = (file: FileEntry) => {
		setTargetFile(file);
		setDialogInput(file.name);
		setDialogType("rename");
		setOperationError(null);
	};

	const openDeleteDialog = (file: FileEntry) => {
		setTargetFile(file);
		setDeleteDialogOpen(true);
		setOperationError(null);
	};

	const openNewFolderDialog = () => {
		setDialogInput("");
		setDialogType("newFolder");
		setOperationError(null);
	};

	const openMoveDialog = (file: FileEntry) => {
		setTargetFile(file);
		setDialogInput(currentPath);
		setDialogType("move");
		setOperationError(null);
	};

	const openCopyDialog = (file: FileEntry) => {
		setTargetFile(file);
		setDialogInput(currentPath);
		setDialogType("copy");
		setOperationError(null);
	};

	const openBulkMoveDialog = () => {
		setDialogInput(currentPath);
		setDialogType("bulkMove");
		setOperationError(null);
	};

	const openBulkCopyDialog = () => {
		setDialogInput(currentPath);
		setDialogType("bulkCopy");
		setOperationError(null);
	};

	const openBulkDeleteDialog = () => {
		setBulkDeleteDialogOpen(true);
		setOperationError(null);
	};

	const closeDialog = () => {
		setDialogType(null);
		setDialogInput("");
		setTargetFile(null);
		setOperationError(null);
	};

	const handleRename = async () => {
		if (!targetFile || !dialogInput.trim()) return;
		try {
			await renameFile(targetFile.path, dialogInput.trim());
			closeDialog();
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleDelete = async (permanent: boolean) => {
		if (!targetFile) return;
		try {
			await deleteFile(targetFile.path, permanent);
			setDeleteDialogOpen(false);
			setTargetFile(null);
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleBulkDelete = async (permanent: boolean) => {
		try {
			await deleteSelected(permanent);
			setBulkDeleteDialogOpen(false);
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleNewFolder = async () => {
		if (!dialogInput.trim()) return;
		try {
			await createFolder(dialogInput.trim());
			closeDialog();
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleMove = async () => {
		if (!targetFile || !dialogInput.trim()) return;
		try {
			await moveFile(targetFile.path, dialogInput.trim());
			closeDialog();
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleCopy = async () => {
		if (!targetFile || !dialogInput.trim()) return;
		try {
			await copyFile(targetFile.path, dialogInput.trim());
			closeDialog();
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleBulkMove = async () => {
		if (!dialogInput.trim()) return;
		try {
			await moveSelected(dialogInput.trim());
			closeDialog();
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleBulkCopy = async () => {
		if (!dialogInput.trim()) return;
		try {
			await copySelected(dialogInput.trim());
			closeDialog();
		} catch (e) {
			setOperationError(String(e));
		}
	};

	const handleReveal = async (file: FileEntry) => {
		try {
			await revealInFolder(file.path);
		} catch (e) {
			console.error("Failed to reveal:", e);
		}
	};

	const handleDialogSubmit = () => {
		switch (dialogType) {
			case "rename":
				handleRename();
				break;
			case "newFolder":
				handleNewFolder();
				break;
			case "move":
				handleMove();
				break;
			case "copy":
				handleCopy();
				break;
			case "bulkMove":
				handleBulkMove();
				break;
			case "bulkCopy":
				handleBulkCopy();
				break;
		}
	};

	const getDialogTitle = () => {
		switch (dialogType) {
			case "rename":
				return "Rename";
			case "newFolder":
				return "New Folder";
			case "move":
				return "Move To";
			case "copy":
				return "Copy To";
			case "bulkMove":
				return `Move ${selectedPaths.size} Items`;
			case "bulkCopy":
				return `Copy ${selectedPaths.size} Items`;
			default:
				return "";
		}
	};

	const getDialogDescription = () => {
		switch (dialogType) {
			case "rename":
				return `Enter a new name for "${targetFile?.name}"`;
			case "newFolder":
				return "Enter a name for the new folder";
			case "move":
				return `Enter destination path to move "${targetFile?.name}"`;
			case "copy":
				return `Enter destination path to copy "${targetFile?.name}"`;
			case "bulkMove":
				return `Enter destination path to move ${selectedPaths.size} selected items`;
			case "bulkCopy":
				return `Enter destination path to copy ${selectedPaths.size} selected items`;
			default:
				return "";
		}
	};

	const getDialogButtonText = () => {
		switch (dialogType) {
			case "rename":
				return "Rename";
			case "newFolder":
				return "Create";
			case "move":
			case "bulkMove":
				return "Move";
			case "copy":
			case "bulkCopy":
				return "Copy";
			default:
				return "OK";
		}
	};

	const handleSelectAllToggle = () => {
		if (allSelected) {
			clearSelection();
		} else {
			selectAll();
		}
	};

	const copyPath = async (path: string) => {
		await writeText(path);
		toast.success("Path copied to clipboard");
	};

	return (
		<div className="flex h-full flex-col">
			{/* Navigation Bar */}
			<div className="flex items-center gap-1 border-border/50 border-b bg-muted/40 px-2 py-1.5">
				<Button
					variant="ghost"
					size="icon-sm"
					onClick={goUp}
					disabled={currentPath === "/"}
					className="h-7 w-7"
				>
					<ChevronLeft className="size-4" />
				</Button>
				<div className="flex min-w-0 flex-1 items-center rounded-md px-2 py-1">
					<span className="truncate text-xs">{currentPath}</span>{" "}
					<button onClick={() => copyPath(currentPath)} type="button">
						<CopyIcon className="ml-2 size-4" />
					</button>
				</div>
				<Button
					variant="ghost"
					size="icon-sm"
					onClick={openNewFolderDialog}
					title="New Folder"
					className="h-7 w-7"
				>
					<FolderPlus className="h-3.5 w-3.5" />
				</Button>
				<Button
					variant="ghost"
					size="icon-sm"
					onClick={refresh}
					disabled={loading}
					title="Refresh"
					className="h-7 w-7"
				>
					<RefreshCw className={cn("h-3.5 w-3.5", loading && "animate-spin")} />
				</Button>
			</div>

			{/* Bulk Actions Bar */}
			{hasSelection && (
				<BulkActionsBar
					selectedCount={selectedPaths.size}
					hasMediaFiles={getMediaFilePaths().length > 0}
					onRename={() => setBulkRenameDialogOpen(true)}
					onCreateFolders={() => setFolderCreationDialogOpen(true)}
					onCleanStreams={() => setBulkStreamCleanupDialogOpen(true)}
					onMove={openBulkMoveDialog}
					onCopy={openBulkCopyDialog}
					onDelete={openBulkDeleteDialog}
					onClearSelection={clearSelection}
				/>
			)}

			{error && (
				<div className="bg-destructive/10 px-3 py-2 text-destructive text-xs">
					{error}
				</div>
			)}

			{/* Filter Panel */}
			<FilterPanel
				currentPath={currentPath}
				isOpen={filterPanelOpen}
				onToggle={() => setFilterPanelOpen(!filterPanelOpen)}
				onFilterApplied={handleFilterApplied}
			/>

			{/* Filtered Results Info */}
			{filteredFiles !== null && (
				<div className="bg-muted/30 px-3 py-1 text-[11px] text-muted-foreground">
					Showing {filteredFiles.length} of {files.length} items
				</div>
			)}

			{/* Column Headers */}
			<div className="grid grid-cols-[24px_20px_1fr_70px_130px] items-center gap-2 border-border/50 border-b px-3 py-1 font-medium text-[11px] text-muted-foreground uppercase tracking-wider">
				<div className="flex items-center justify-center">
					<Checkbox
						checked={allSelected}
						onCheckedChange={handleSelectAllToggle}
						aria-label="Select all"
						className="h-3.5 w-3.5"
					/>
				</div>
				<span />
				<span>Name</span>
				<span className="text-right">Size</span>
				<span className="text-right">Modified</span>
			</div>

			<ScrollArea className="h-[calc(100%-6rem)] flex-1">
				<div className="px-1 py-0.5">
					{displayedFiles.map((file) => (
						<ContextMenu key={file.path}>
							<ContextMenuTrigger asChild>
								{/** biome-ignore lint/a11y/noStaticElementInteractions: need for context menu */}
								<div
									// biome-ignore lint/a11y/noNoninteractiveTabindex: need for context menu
									tabIndex={0}
									className={cn(
										"grid w-full cursor-pointer grid-cols-[24px_20px_1fr_70px_130px] items-center gap-2 rounded px-3 py-1 text-[13px] transition-colors",
										"hover:bg-accent/50",
										selectedPath === file.path && "bg-accent",
										selectedPaths.has(file.path) &&
											"bg-primary/10 hover:bg-primary/15",
										!file.is_dir &&
											!file.is_media &&
											"opacity-40 hover:opacity-60",
										file.is_dir && "font-medium",
									)}
									onClick={() => selectFile(file)}
									onDoubleClick={() => navigate(file)}
									onKeyDown={(e) => {
										if (e.key === "Enter" || e.key === " ") {
											e.preventDefault();
											selectFile(file);
										}
									}}
								>
									<span className="flex items-center justify-center">
										<Checkbox
											checked={selectedPaths.has(file.path)}
											onCheckedChange={() => toggleSelection(file.path)}
											onClick={(e) => e.stopPropagation()}
											aria-label={`Select ${file.name}`}
											className="h-3.5 w-3.5"
										/>
									</span>
									<span className="text-muted-foreground/70">
										{file.is_dir ? (
											<Folder className="size-4 text-blue-500/80" />
										) : file.is_media ? (
											<Film className="size-4 text-purple-500/80" />
										) : (
											<FileText className="size-4" />
										)}
									</span>
									<span className="truncate text-left">{file.name}</span>
									<span className="text-right text-[11px] text-muted-foreground/70 tabular-nums">
										{file.is_dir ? "—" : formatSize(file.size)}
									</span>
									<span className="text-right text-[11px] text-muted-foreground/70">
										{file.modified || "—"}
									</span>
								</div>
							</ContextMenuTrigger>
							<ContextMenuContent className="w-48">
								{file.is_dir && (
									<>
										<ContextMenuItem onClick={() => navigate(file)}>
											<FolderOpen className="mr-2 size-4" />
											Open
										</ContextMenuItem>
										<ContextMenuSeparator />
									</>
								)}
								<ContextMenuItem onClick={() => openRenameDialog(file)}>
									<Pencil className="mr-2 size-4" />
									Rename
								</ContextMenuItem>
								<ContextMenuItem onClick={() => openMoveDialog(file)}>
									<Move className="mr-2 size-4" />
									Move To...
								</ContextMenuItem>
								<ContextMenuItem onClick={() => openCopyDialog(file)}>
									<Copy className="mr-2 size-4" />
									Copy To...
								</ContextMenuItem>
								<ContextMenuSeparator />
								<ContextMenuItem onClick={() => handleReveal(file)}>
									<FolderOpen className="mr-2 size-4" />
									Reveal in Finder
								</ContextMenuItem>
								<ContextMenuSeparator />
								<ContextMenuItem
									onClick={() => openDeleteDialog(file)}
									className="text-destructive focus:text-destructive"
								>
									<Trash className="mr-2 size-4" />
									Delete
								</ContextMenuItem>
							</ContextMenuContent>
						</ContextMenu>
					))}
				</div>
			</ScrollArea>

			{/* Rename / New Folder / Move / Copy Dialog */}
			<Dialog open={dialogType !== null} onOpenChange={() => closeDialog()}>
				<DialogContent>
					<DialogHeader>
						<DialogTitle>{getDialogTitle()}</DialogTitle>
						<DialogDescription>{getDialogDescription()}</DialogDescription>
					</DialogHeader>
					<div className="py-4">
						<div className="flex gap-2">
							<Input
								value={dialogInput}
								onChange={(e) => setDialogInput(e.target.value)}
								placeholder={
									dialogType === "rename" || dialogType === "newFolder"
										? "Name"
										: "Path"
								}
								onKeyDown={(e) => {
									if (e.key === "Enter") {
										handleDialogSubmit();
									}
								}}
								className="flex-1"
							/>
							{(dialogType === "move" ||
								dialogType === "copy" ||
								dialogType === "bulkMove" ||
								dialogType === "bulkCopy") && (
								<Button
									variant="outline"
									size="icon"
									onClick={async () => {
										try {
											const selected = await invoke<string | null>(
												"pick_folder",
											);
											if (selected) {
												setDialogInput(selected);
											}
										} catch (e) {
											console.error("Folder picker error:", e);
											toast.error(String(e));
										}
									}}
									title="Browse for folder"
								>
									<FolderOpen className="size-4" />
								</Button>
							)}
						</div>
						{operationError && (
							<p className="mt-2 text-destructive text-sm">{operationError}</p>
						)}
					</div>
					<DialogFooter>
						<Button variant="outline" onClick={closeDialog}>
							Cancel
						</Button>
						<Button onClick={handleDialogSubmit}>
							{getDialogButtonText()}
						</Button>
					</DialogFooter>
				</DialogContent>
			</Dialog>

			{/* Single Delete Confirmation Dialog */}
			<AlertDialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Delete "{targetFile?.name}"?</AlertDialogTitle>
						<AlertDialogDescription>
							Choose how you want to delete this{" "}
							{targetFile?.is_dir ? "folder" : "file"}. Moving to trash allows
							recovery, while permanent deletion cannot be undone.
						</AlertDialogDescription>
					</AlertDialogHeader>
					{operationError && (
						<p className="text-destructive text-sm">{operationError}</p>
					)}
					<AlertDialogFooter className="flex-col gap-2 sm:flex-row">
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction
							onClick={() => handleDelete(false)}
							className="bg-secondary text-secondary-foreground hover:bg-secondary/80"
						>
							<Trash className="mr-2 size-4" />
							Move to Trash
						</AlertDialogAction>
						<AlertDialogAction
							onClick={() => handleDelete(true)}
							className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
						>
							<Trash2 className="mr-2 size-4" />
							Delete Permanently
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>

			{/* Bulk Delete Confirmation Dialog */}
			<AlertDialog
				open={bulkDeleteDialogOpen}
				onOpenChange={setBulkDeleteDialogOpen}
			>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>
							Delete {selectedPaths.size} items?
						</AlertDialogTitle>
						<AlertDialogDescription>
							Choose how you want to delete these items. Moving to trash allows
							recovery, while permanent deletion cannot be undone.
						</AlertDialogDescription>
					</AlertDialogHeader>
					{operationError && (
						<p className="text-destructive text-sm">{operationError}</p>
					)}
					<AlertDialogFooter className="flex-col gap-2 sm:flex-row">
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction
							onClick={() => handleBulkDelete(false)}
							className="bg-secondary text-secondary-foreground hover:bg-secondary/80"
						>
							<Trash className="mr-2 size-4" />
							Move to Trash
						</AlertDialogAction>
						<AlertDialogAction
							onClick={() => handleBulkDelete(true)}
							className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
						>
							<Trash2 className="mr-2 size-4" />
							Delete Permanently
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>

			{/* Bulk Rename Dialog */}
			<BulkRenameDialog
				open={bulkRenameDialogOpen}
				onOpenChange={setBulkRenameDialogOpen}
				paths={getBulkRenameablePaths()}
				onSuccess={() => {
					clearSelection();
					refresh();
				}}
			/>

			{/* Bulk Stream Cleanup Dialog */}
			<BulkStreamCleanupDialog
				open={bulkStreamCleanupDialogOpen}
				onOpenChange={setBulkStreamCleanupDialogOpen}
				paths={getMediaFilePaths()}
				onSuccess={() => {
					clearSelection();
					refresh();
				}}
			/>

			{/* Folder Creation Dialog */}
			<FolderCreationDialog
				open={folderCreationDialogOpen}
				onOpenChange={setFolderCreationDialogOpen}
				paths={getBulkRenameablePaths()}
				currentPath={currentPath}
				onSuccess={() => {
					clearSelection();
					refresh();
				}}
			/>
		</div>
	);
}
