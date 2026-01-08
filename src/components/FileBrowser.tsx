import { useCallback, useState } from "react";
import { BulkActionsBar } from "@/components/BulkActionsBar";
import { BulkRenameDialog } from "@/components/BulkRenameDialog";
import { BulkStreamCleanupDialog } from "@/components/BulkStreamCleanupDialog";
import { FilterPanel } from "@/components/FilterPanel";
import { FolderCreationDialog } from "@/components/FolderCreationDialog";
import {
	BulkDeleteDialog,
	FileBrowserToolbar,
	FileListHeader,
	FileListItem,
	FileOperationDialog,
	SingleDeleteDialog,
	useFileDialogs,
} from "@/components/file-browser";
import { ScrollArea } from "@/components/ui/scroll-area";
import { type FileEntry, useFileBrowserStore } from "@/stores/fileBrowserStore";

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

	const dialogs = useFileDialogs();
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

	const hasSelection = selectedPaths.size > 0;
	const allSelected = files.length > 0 && selectedPaths.size === files.length;

	// Operation handlers
	const handleRename = async () => {
		if (!dialogs.targetFile || !dialogs.dialogInput.trim()) return;
		try {
			await renameFile(dialogs.targetFile.path, dialogs.dialogInput.trim());
			dialogs.closeDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
		}
	};

	const handleDelete = async (permanent: boolean) => {
		if (!dialogs.targetFile) return;
		try {
			await deleteFile(dialogs.targetFile.path, permanent);
			dialogs.closeDeleteDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
		}
	};

	const handleBulkDelete = async (permanent: boolean) => {
		try {
			await deleteSelected(permanent);
			dialogs.closeBulkDeleteDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
		}
	};

	const handleNewFolder = async () => {
		if (!dialogs.dialogInput.trim()) return;
		try {
			await createFolder(dialogs.dialogInput.trim());
			dialogs.closeDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
		}
	};

	const handleMove = async () => {
		if (!dialogs.targetFile || !dialogs.dialogInput.trim()) return;
		try {
			await moveFile(dialogs.targetFile.path, dialogs.dialogInput.trim());
			dialogs.closeDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
		}
	};

	const handleCopy = async () => {
		if (!dialogs.targetFile || !dialogs.dialogInput.trim()) return;
		try {
			await copyFile(dialogs.targetFile.path, dialogs.dialogInput.trim());
			dialogs.closeDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
		}
	};

	const handleBulkMove = async () => {
		if (!dialogs.dialogInput.trim()) return;
		try {
			await moveSelected(dialogs.dialogInput.trim());
			dialogs.closeDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
		}
	};

	const handleBulkCopy = async () => {
		if (!dialogs.dialogInput.trim()) return;
		try {
			await copySelected(dialogs.dialogInput.trim());
			dialogs.closeDialog();
		} catch (e) {
			dialogs.setOperationError(String(e));
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
		switch (dialogs.dialogType) {
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

	const handleSelectAllToggle = () => {
		if (allSelected) {
			clearSelection();
		} else {
			selectAll();
		}
	};

	return (
		<div className="flex h-full flex-col">
			{/* Navigation Bar */}
			<FileBrowserToolbar
				currentPath={currentPath}
				loading={loading}
				onGoUp={goUp}
				onRefresh={refresh}
				onNewFolder={dialogs.openNewFolderDialog}
			/>

			{/* Bulk Actions Bar */}
			{hasSelection && (
				<BulkActionsBar
					selectedCount={selectedPaths.size}
					hasMediaFiles={getMediaFilePaths().length > 0}
					onRename={dialogs.openBulkRenameDialog}
					onCreateFolders={dialogs.openFolderCreationDialog}
					onCleanStreams={dialogs.openBulkStreamCleanupDialog}
					onMove={() => dialogs.openBulkMoveDialog(currentPath)}
					onCopy={() => dialogs.openBulkCopyDialog(currentPath)}
					onDelete={dialogs.openBulkDeleteDialog}
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
			<FileListHeader
				allSelected={allSelected}
				onSelectAllToggle={handleSelectAllToggle}
			/>

			{/* File List */}
			<ScrollArea className="h-[calc(100%-6rem)] flex-1">
				<div className="px-1 py-0.5">
					{displayedFiles.map((file) => (
						<FileListItem
							key={file.path}
							file={file}
							isSelected={selectedPath === file.path}
							isChecked={selectedPaths.has(file.path)}
							onSelect={() => selectFile(file)}
							onNavigate={() => navigate(file)}
							onToggleSelection={() => toggleSelection(file.path)}
							onRename={() => dialogs.openRenameDialog(file)}
							onMove={() => dialogs.openMoveDialog(file, currentPath)}
							onCopy={() => dialogs.openCopyDialog(file, currentPath)}
							onDelete={() => dialogs.openDeleteDialog(file)}
							onReveal={() => handleReveal(file)}
						/>
					))}
				</div>
			</ScrollArea>

			{/* File Operation Dialog (Rename / New Folder / Move / Copy) */}
			<FileOperationDialog
				dialogType={dialogs.dialogType}
				dialogInput={dialogs.dialogInput}
				targetFileName={dialogs.targetFile?.name}
				selectedCount={selectedPaths.size}
				operationError={dialogs.operationError}
				onInputChange={dialogs.setDialogInput}
				onSubmit={handleDialogSubmit}
				onClose={dialogs.closeDialog}
			/>

			{/* Single Delete Confirmation Dialog */}
			<SingleDeleteDialog
				open={dialogs.deleteDialogOpen}
				onOpenChange={(open) => !open && dialogs.closeDeleteDialog()}
				fileName={dialogs.targetFile?.name}
				isDir={dialogs.targetFile?.is_dir}
				operationError={dialogs.operationError}
				onDelete={handleDelete}
			/>

			{/* Bulk Delete Confirmation Dialog */}
			<BulkDeleteDialog
				open={dialogs.bulkDeleteDialogOpen}
				onOpenChange={(open) => !open && dialogs.closeBulkDeleteDialog()}
				selectedCount={selectedPaths.size}
				operationError={dialogs.operationError}
				onDelete={handleBulkDelete}
			/>

			{/* Bulk Rename Dialog */}
			<BulkRenameDialog
				open={dialogs.bulkRenameDialogOpen}
				onOpenChange={(open) => !open && dialogs.closeBulkRenameDialog()}
				paths={getBulkRenameablePaths()}
				onSuccess={() => {
					clearSelection();
					refresh();
				}}
			/>

			{/* Bulk Stream Cleanup Dialog */}
			<BulkStreamCleanupDialog
				open={dialogs.bulkStreamCleanupDialogOpen}
				onOpenChange={(open) => !open && dialogs.closeBulkStreamCleanupDialog()}
				paths={getMediaFilePaths()}
				onSuccess={() => {
					clearSelection();
					refresh();
				}}
			/>

			{/* Folder Creation Dialog */}
			<FolderCreationDialog
				open={dialogs.folderCreationDialogOpen}
				onOpenChange={(open) => !open && dialogs.closeFolderCreationDialog()}
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
