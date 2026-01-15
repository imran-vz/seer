import {
	Copy,
	FileText,
	Film,
	Folder,
	FolderOpen,
	Move,
	Pencil,
	Trash,
} from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import {
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuSeparator,
	ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { formatSize } from "@/lib/fileUtils";
import { cn } from "@/lib/utils";
import type { FileEntry } from "@/stores/fileBrowserStore";

interface FileListItemProps {
	file: FileEntry;
	isSelected: boolean;
	isChecked: boolean;
	onSelect: () => void;
	onNavigate: () => void;
	onToggleSelection: () => void;
	onRename: () => void;
	onMove: () => void;
	onCopy: () => void;
	onDelete: () => void;
	onReveal: () => void;
}

export function FileListItem({
	file,
	isSelected,
	isChecked,
	onSelect,
	onNavigate,
	onToggleSelection,
	onRename,
	onMove,
	onCopy,
	onDelete,
	onReveal,
}: FileListItemProps) {
	return (
		<ContextMenu>
			<ContextMenuTrigger asChild>
				{/** biome-ignore lint/a11y/noStaticElementInteractions: need for context menu */}
				<div
					// biome-ignore lint/a11y/noNoninteractiveTabindex: need for context menu
					tabIndex={0}
					className={cn(
						"grid w-full cursor-pointer grid-cols-[24px_20px_1fr_70px_130px] items-center gap-2 rounded px-3 py-1 text-[13px] transition-colors",
						"hover:bg-accent/50",
						isSelected && "bg-accent",
						isChecked && "bg-primary/10 hover:bg-primary/15",
						!file.is_dir && !file.is_media && "opacity-40 hover:opacity-60",
						file.is_dir && "font-medium",
					)}
					onClick={onSelect}
					onDoubleClick={onNavigate}
					onKeyDown={(e) => {
						if (e.key === "Enter" || e.key === " ") {
							e.preventDefault();
							onSelect();
						}
					}}
				>
					<span className="flex items-center justify-center">
						<Checkbox
							checked={isChecked}
							onCheckedChange={onToggleSelection}
							onClick={(e) => e.stopPropagation()}
							aria-label={`Select ${file.name}`}
							className="h-3.5 w-3.5"
						/>
					</span>
					<span className="text-muted-foreground/70">
						{file.is_dir ? (
							<Folder className="size-4" />
						) : file.is_media ? (
							<Film className="size-4" />
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
						<ContextMenuItem onClick={onNavigate}>
							<FolderOpen className="mr-2 size-4" />
							Open
						</ContextMenuItem>
						<ContextMenuSeparator />
					</>
				)}
				<ContextMenuItem onClick={onRename}>
					<Pencil className="mr-2 size-4" />
					Rename
				</ContextMenuItem>
				<ContextMenuItem onClick={onMove}>
					<Move className="mr-2 size-4" />
					Move To...
				</ContextMenuItem>
				<ContextMenuItem onClick={onCopy}>
					<Copy className="mr-2 size-4" />
					Copy To...
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem onClick={onReveal}>
					<FolderOpen className="mr-2 size-4" />
					Reveal in Finder
				</ContextMenuItem>
				<ContextMenuSeparator />
				<ContextMenuItem
					onClick={onDelete}
					className="text-destructive focus:text-destructive"
				>
					<Trash className="mr-2 size-4" />
					Delete
				</ContextMenuItem>
			</ContextMenuContent>
		</ContextMenu>
	);
}
