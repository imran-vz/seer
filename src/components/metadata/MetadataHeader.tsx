import {
	AlertTriangle,
	Archive,
	Clock,
	FileVideo,
	RotateCcw,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import type {
	MetadataSnapshot,
	MetadataToolAvailability,
} from "@/types/metadata";

interface MetadataHeaderProps {
	snapshot: MetadataSnapshot;
	toolAvailability: MetadataToolAvailability | null;
	onRefresh: () => void;
}

export function MetadataHeader({
	snapshot,
	toolAvailability,
	onRefresh,
}: MetadataHeaderProps) {
	const toolsReady = toolAvailability?.ffmpeg || toolAvailability?.exiftool;

	const formatSize = (bytes: number): string => {
		const units = ["B", "KB", "MB", "GB", "TB"];
		const i = Math.min(
			units.length - 1,
			Math.floor(Math.log(bytes || 1) / Math.log(1024)),
		);
		return `${(bytes / 1024 ** i).toFixed(2)} ${units[i]}`;
	};

	return (
		<div className="border-border border-b bg-card p-4">
			<div className="mb-4 flex items-start justify-between gap-4">
				<div className="min-w-0 flex-1">
					<h2
						className="truncate font-semibold text-lg"
						title={snapshot.fileName}
					>
						{snapshot.fileName}
					</h2>
					<p
						className="truncate text-muted-foreground text-xs"
						title={snapshot.path}
					>
						{snapshot.path}
					</p>
				</div>
				<div className="flex items-center gap-1">
					{!toolsReady && (
						<TooltipProvider>
							<Tooltip>
								<TooltipTrigger asChild>
									<AlertTriangle className="mr-2 h-4 w-4 text-amber-500" />
								</TooltipTrigger>
								<TooltipContent>
									FFmpeg/ExifTool missing. Editing disabled.
								</TooltipContent>
							</Tooltip>
						</TooltipProvider>
					)}
					<Button
						variant="ghost"
						size="icon"
						onClick={onRefresh}
						title="Refresh Metadata"
					>
						<RotateCcw className="h-4 w-4" />
					</Button>
				</div>
			</div>

			<div className="flex flex-wrap gap-4 text-sm">
				<div className="flex items-center gap-2">
					<div className="flex h-8 w-8 items-center justify-center rounded-md bg-primary/10 text-primary">
						<Archive className="h-4 w-4" />
					</div>
					<div className="flex flex-col">
						<span className="text-[10px] text-muted-foreground uppercase">
							Size
						</span>
						<span className="font-medium">{formatSize(snapshot.size)}</span>
					</div>
				</div>
				<div className="flex items-center gap-2">
					<div className="flex h-8 w-8 items-center justify-center rounded-md bg-primary/10 text-primary">
						<FileVideo className="h-4 w-4" />
					</div>
					<div className="flex flex-col">
						<span className="text-[10px] text-muted-foreground uppercase">
							Type
						</span>
						<span className="font-medium">
							{snapshot.extension?.toUpperCase() || "UNK"}
						</span>
					</div>
				</div>
				<div className="flex items-center gap-2">
					<div className="flex h-8 w-8 items-center justify-center rounded-md bg-primary/10 text-primary">
						<Clock className="h-4 w-4" />
					</div>
					<div className="flex flex-col">
						<span className="text-[10px] text-muted-foreground uppercase">
							Modified
						</span>
						<span className="font-medium">{snapshot.modified || "-"}</span>
					</div>
				</div>
			</div>
		</div>
	);
}
