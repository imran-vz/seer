import { Trash, Trash2 } from "lucide-react";
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

interface SingleDeleteDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	fileName?: string;
	isDir?: boolean;
	operationError: string | null;
	onDelete: (permanent: boolean) => void;
}

export function SingleDeleteDialog({
	open,
	onOpenChange,
	fileName,
	isDir,
	operationError,
	onDelete,
}: SingleDeleteDialogProps) {
	return (
		<AlertDialog open={open} onOpenChange={onOpenChange}>
			<AlertDialogContent>
				<AlertDialogHeader>
					<AlertDialogTitle>Delete "{fileName}"?</AlertDialogTitle>
					<AlertDialogDescription>
						Choose how you want to delete this {isDir ? "folder" : "file"}.
						Moving to trash allows recovery, while permanent deletion cannot be
						undone.
					</AlertDialogDescription>
				</AlertDialogHeader>
				{operationError && (
					<p className="text-destructive text-sm">{operationError}</p>
				)}
				<AlertDialogFooter className="flex-col gap-2 sm:flex-row">
					<AlertDialogCancel>Cancel</AlertDialogCancel>
					<AlertDialogAction
						onClick={() => onDelete(false)}
						className="bg-secondary text-secondary-foreground hover:bg-secondary/80"
					>
						<Trash className="mr-2 size-4" />
						Move to Trash
					</AlertDialogAction>
					<AlertDialogAction
						onClick={() => onDelete(true)}
						className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
					>
						<Trash2 className="mr-2 size-4" />
						Delete Permanently
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}

interface BulkDeleteDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	selectedCount: number;
	operationError: string | null;
	onDelete: (permanent: boolean) => void;
}

export function BulkDeleteDialog({
	open,
	onOpenChange,
	selectedCount,
	operationError,
	onDelete,
}: BulkDeleteDialogProps) {
	return (
		<AlertDialog open={open} onOpenChange={onOpenChange}>
			<AlertDialogContent>
				<AlertDialogHeader>
					<AlertDialogTitle>Delete {selectedCount} items?</AlertDialogTitle>
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
						onClick={() => onDelete(false)}
						className="bg-secondary text-secondary-foreground hover:bg-secondary/80"
					>
						<Trash className="mr-2 size-4" />
						Move to Trash
					</AlertDialogAction>
					<AlertDialogAction
						onClick={() => onDelete(true)}
						className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
					>
						<Trash2 className="mr-2 size-4" />
						Delete Permanently
					</AlertDialogAction>
				</AlertDialogFooter>
			</AlertDialogContent>
		</AlertDialog>
	);
}
