import { invoke } from "@tauri-apps/api/core";
import { FolderOpen } from "lucide-react";
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
import type { DialogType } from "./useFileDialogs";

interface FileOperationDialogProps {
	dialogType: DialogType;
	dialogInput: string;
	targetFileName?: string;
	selectedCount?: number;
	operationError: string | null;
	onInputChange: (value: string) => void;
	onSubmit: () => void;
	onClose: () => void;
}

export function FileOperationDialog({
	dialogType,
	dialogInput,
	targetFileName,
	selectedCount = 0,
	operationError,
	onInputChange,
	onSubmit,
	onClose,
}: FileOperationDialogProps) {
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
				return `Move ${selectedCount} Items`;
			case "bulkCopy":
				return `Copy ${selectedCount} Items`;
			default:
				return "";
		}
	};

	const getDialogDescription = () => {
		switch (dialogType) {
			case "rename":
				return `Enter a new name for "${targetFileName}"`;
			case "newFolder":
				return "Enter a name for the new folder";
			case "move":
				return `Enter destination path to move "${targetFileName}"`;
			case "copy":
				return `Enter destination path to copy "${targetFileName}"`;
			case "bulkMove":
				return `Enter destination path to move ${selectedCount} selected items`;
			case "bulkCopy":
				return `Enter destination path to copy ${selectedCount} selected items`;
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

	const showFolderPicker =
		dialogType === "move" ||
		dialogType === "copy" ||
		dialogType === "bulkMove" ||
		dialogType === "bulkCopy";

	const handleFolderPick = async () => {
		try {
			const selected = await invoke<string | null>("pick_folder");
			if (selected) {
				onInputChange(selected);
			}
		} catch (e) {
			console.error("Folder picker error:", e);
			toast.error(String(e));
		}
	};

	return (
		<Dialog open={dialogType !== null} onOpenChange={() => onClose()}>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>{getDialogTitle()}</DialogTitle>
					<DialogDescription>{getDialogDescription()}</DialogDescription>
				</DialogHeader>
				<div className="py-4">
					<div className="flex gap-2">
						<Input
							value={dialogInput}
							onChange={(e) => onInputChange(e.target.value)}
							placeholder={
								dialogType === "rename" || dialogType === "newFolder"
									? "Name"
									: "Path"
							}
							onKeyDown={(e) => {
								if (e.key === "Enter") {
									onSubmit();
								}
							}}
							className="flex-1"
						/>
						{showFolderPicker && (
							<Button
								variant="outline"
								size="icon"
								onClick={handleFolderPick}
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
					<Button variant="outline" onClick={onClose}>
						Cancel
					</Button>
					<Button onClick={onSubmit}>{getDialogButtonText()}</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
