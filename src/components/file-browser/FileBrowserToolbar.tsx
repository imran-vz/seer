import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { ChevronLeft, CopyIcon, FolderPlus, RefreshCw } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

interface FileBrowserToolbarProps {
	currentPath: string;
	loading: boolean;
	onGoUp: () => void;
	onRefresh: () => void;
	onNewFolder: () => void;
}

export function FileBrowserToolbar({
	currentPath,
	loading,
	onGoUp,
	onRefresh,
	onNewFolder,
}: FileBrowserToolbarProps) {
	const copyPath = async (path: string) => {
		await writeText(path);
		toast.success("Path copied to clipboard");
	};

	return (
		<div className="flex items-center gap-1 border-border/50 border-b bg-muted/40 px-2 py-1.5">
			<Button
				variant="ghost"
				size="icon-sm"
				onClick={onGoUp}
				disabled={currentPath === "/"}
				className="h-7 w-7"
			>
				<ChevronLeft className="size-4" />
			</Button>
			<div className="flex min-w-0 flex-1 items-center rounded-md px-2 py-1">
				<span className="truncate text-xs">{currentPath}</span>
				<button onClick={() => copyPath(currentPath)} type="button">
					<CopyIcon className="ml-2 size-4" />
				</button>
			</div>

			<Tooltip>
				<TooltipTrigger asChild>
					<Button
						variant="ghost"
						size="icon-sm"
						onClick={onNewFolder}
						title="New Folder"
						className="h-7 w-7"
					>
						<FolderPlus className="h-3.5 w-3.5" />
					</Button>
				</TooltipTrigger>
				<TooltipContent>Create Folder</TooltipContent>
			</Tooltip>

			<Tooltip>
				<TooltipTrigger asChild>
					<Button
						variant="ghost"
						size="icon-sm"
						onClick={onRefresh}
						disabled={loading}
						title="Refresh"
						className="h-7 w-7"
					>
						<RefreshCw
							className={cn("h-3.5 w-3.5", loading && "animate-spin")}
						/>
					</Button>
				</TooltipTrigger>
				<TooltipContent>Refresh</TooltipContent>
			</Tooltip>
		</div>
	);
}
