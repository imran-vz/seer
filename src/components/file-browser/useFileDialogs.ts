import { useCallback, useState } from "react";
import type { FileEntry } from "@/stores/fileBrowserStore";

export type DialogType =
	| "rename"
	| "newFolder"
	| "move"
	| "copy"
	| "bulkMove"
	| "bulkCopy"
	| null;

export interface FileDialogsState {
	dialogType: DialogType;
	dialogInput: string;
	targetFile: FileEntry | null;
	deleteDialogOpen: boolean;
	bulkDeleteDialogOpen: boolean;
	bulkRenameDialogOpen: boolean;
	bulkStreamCleanupDialogOpen: boolean;
	folderCreationDialogOpen: boolean;
	operationError: string | null;
}

export interface FileDialogsActions {
	setDialogInput: (input: string) => void;
	setOperationError: (error: string | null) => void;
	openRenameDialog: (file: FileEntry) => void;
	openDeleteDialog: (file: FileEntry) => void;
	openNewFolderDialog: () => void;
	openMoveDialog: (file: FileEntry, currentPath: string) => void;
	openCopyDialog: (file: FileEntry, currentPath: string) => void;
	openBulkMoveDialog: (currentPath: string) => void;
	openBulkCopyDialog: (currentPath: string) => void;
	openBulkDeleteDialog: () => void;
	openBulkRenameDialog: () => void;
	openBulkStreamCleanupDialog: () => void;
	openFolderCreationDialog: () => void;
	closeDialog: () => void;
	closeDeleteDialog: () => void;
	closeBulkDeleteDialog: () => void;
	closeBulkRenameDialog: () => void;
	closeBulkStreamCleanupDialog: () => void;
	closeFolderCreationDialog: () => void;
}

export function useFileDialogs(): FileDialogsState & FileDialogsActions {
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

	const openRenameDialog = useCallback((file: FileEntry) => {
		setTargetFile(file);
		setDialogInput(file.name);
		setDialogType("rename");
		setOperationError(null);
	}, []);

	const openDeleteDialog = useCallback((file: FileEntry) => {
		setTargetFile(file);
		setDeleteDialogOpen(true);
		setOperationError(null);
	}, []);

	const openNewFolderDialog = useCallback(() => {
		setDialogInput("");
		setDialogType("newFolder");
		setOperationError(null);
	}, []);

	const openMoveDialog = useCallback((file: FileEntry, currentPath: string) => {
		setTargetFile(file);
		setDialogInput(currentPath);
		setDialogType("move");
		setOperationError(null);
	}, []);

	const openCopyDialog = useCallback((file: FileEntry, currentPath: string) => {
		setTargetFile(file);
		setDialogInput(currentPath);
		setDialogType("copy");
		setOperationError(null);
	}, []);

	const openBulkMoveDialog = useCallback((currentPath: string) => {
		setDialogInput(currentPath);
		setDialogType("bulkMove");
		setOperationError(null);
	}, []);

	const openBulkCopyDialog = useCallback((currentPath: string) => {
		setDialogInput(currentPath);
		setDialogType("bulkCopy");
		setOperationError(null);
	}, []);

	const openBulkDeleteDialog = useCallback(() => {
		setBulkDeleteDialogOpen(true);
		setOperationError(null);
	}, []);

	const openBulkRenameDialog = useCallback(() => {
		setBulkRenameDialogOpen(true);
	}, []);

	const openBulkStreamCleanupDialog = useCallback(() => {
		setBulkStreamCleanupDialogOpen(true);
	}, []);

	const openFolderCreationDialog = useCallback(() => {
		setFolderCreationDialogOpen(true);
	}, []);

	const closeDialog = useCallback(() => {
		setDialogType(null);
		setDialogInput("");
		setTargetFile(null);
		setOperationError(null);
	}, []);

	const closeDeleteDialog = useCallback(() => {
		setDeleteDialogOpen(false);
		setTargetFile(null);
		setOperationError(null);
	}, []);

	const closeBulkDeleteDialog = useCallback(() => {
		setBulkDeleteDialogOpen(false);
		setOperationError(null);
	}, []);

	const closeBulkRenameDialog = useCallback(() => {
		setBulkRenameDialogOpen(false);
	}, []);

	const closeBulkStreamCleanupDialog = useCallback(() => {
		setBulkStreamCleanupDialogOpen(false);
	}, []);

	const closeFolderCreationDialog = useCallback(() => {
		setFolderCreationDialogOpen(false);
	}, []);

	return {
		// State
		dialogType,
		dialogInput,
		targetFile,
		deleteDialogOpen,
		bulkDeleteDialogOpen,
		bulkRenameDialogOpen,
		bulkStreamCleanupDialogOpen,
		folderCreationDialogOpen,
		operationError,
		// Actions
		setDialogInput,
		setOperationError,
		openRenameDialog,
		openDeleteDialog,
		openNewFolderDialog,
		openMoveDialog,
		openCopyDialog,
		openBulkMoveDialog,
		openBulkCopyDialog,
		openBulkDeleteDialog,
		openBulkRenameDialog,
		openBulkStreamCleanupDialog,
		openFolderCreationDialog,
		closeDialog,
		closeDeleteDialog,
		closeBulkDeleteDialog,
		closeBulkRenameDialog,
		closeBulkStreamCleanupDialog,
		closeFolderCreationDialog,
	};
}
