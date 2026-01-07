import {
	Copy,
	Film,
	FolderPlus,
	Move,
	TextCursorInput,
	Trash,
	X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";

interface BulkActionsBarProps {
	selectedCount: number;
	hasMediaFiles: boolean;
	onRename: () => void;
	onCreateFolders: () => void;
	onCleanStreams: () => void;
	onMove: () => void;
	onCopy: () => void;
	onDelete: () => void;
	onClearSelection: () => void;
}

export function BulkActionsBar({
	selectedCount,
	hasMediaFiles,
	onRename,
	onCreateFolders,
	onCleanStreams,
	onMove,
	onCopy,
	onDelete,
	onClearSelection,
}: BulkActionsBarProps) {
	return (
		<div className="flex items-center gap-1.5 border-border border-b bg-primary/5 px-2 py-1.5">
			<span className="shrink-0 px-1 font-medium text-primary text-xs">
				{selectedCount} selected
			</span>
			<div className="flex-1" />

			<TooltipProvider delayDuration={300}>
				<div className="flex items-center gap-0.5">
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="ghost"
								size="icon-sm"
								onClick={onRename}
								className="h-7 w-7"
							>
								<TextCursorInput className="h-3.5 w-3.5" />
							</Button>
						</TooltipTrigger>
						<TooltipContent>Rename</TooltipContent>
					</Tooltip>
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="ghost"
								size="icon-sm"
								onClick={onCreateFolders}
								className="h-7 w-7"
							>
								<FolderPlus className="h-3.5 w-3.5" />
							</Button>
						</TooltipTrigger>
						<TooltipContent>Create Folders</TooltipContent>
					</Tooltip>
					{hasMediaFiles && (
						<Tooltip>
							<TooltipTrigger asChild>
								<Button
									variant="ghost"
									size="icon-sm"
									onClick={onCleanStreams}
									className="h-7 w-7"
								>
									<Film className="h-3.5 w-3.5" />
								</Button>
							</TooltipTrigger>
							<TooltipContent>Clean Streams</TooltipContent>
						</Tooltip>
					)}
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="ghost"
								size="icon-sm"
								onClick={onMove}
								className="h-7 w-7"
							>
								<Move className="h-3.5 w-3.5" />
							</Button>
						</TooltipTrigger>
						<TooltipContent>Move</TooltipContent>
					</Tooltip>
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="ghost"
								size="icon-sm"
								onClick={onCopy}
								className="h-7 w-7"
							>
								<Copy className="h-3.5 w-3.5" />
							</Button>
						</TooltipTrigger>
						<TooltipContent>Copy</TooltipContent>
					</Tooltip>
					<Tooltip>
						<TooltipTrigger asChild>
							<Button
								variant="ghost"
								size="icon-sm"
								onClick={onDelete}
								className="h-7 w-7 text-destructive hover:text-destructive"
							>
								<Trash className="h-3.5 w-3.5" />
							</Button>
						</TooltipTrigger>
						<TooltipContent>Delete</TooltipContent>
					</Tooltip>
				</div>
			</TooltipProvider>

			<Tooltip>
				<TooltipTrigger asChild>
					<Button
						variant="ghost"
						size="icon-sm"
						onClick={onClearSelection}
						className="h-7 w-7"
					>
						<X className="h-3.5 w-3.5" />
					</Button>
				</TooltipTrigger>
				<TooltipContent>Clear selection</TooltipContent>
			</Tooltip>
		</div>
	);
}
