import { invoke } from "@tauri-apps/api/core";
import { ChevronLeft, FileText, Film, Folder } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

interface FileEntry {
	name: string;
	path: string;
	is_dir: boolean;
	is_media: boolean;
	size: number;
	modified: string | null;
}

interface FileBrowserProps {
	onFileSelect: (path: string) => void;
}

export function FileBrowser({ onFileSelect }: FileBrowserProps) {
	const [currentPath, setCurrentPath] = useState<string>("");
	const [files, setFiles] = useState<FileEntry[]>([]);
	const [error, setError] = useState<string | null>(null);
	const [selectedPath, setSelectedPath] = useState<string | null>(null);

	const loadDirectory = useCallback(async (path: string) => {
		try {
			setError(null);
			const entries = await invoke<FileEntry[]>("list_directory", { path });
			setFiles(entries);
		} catch (e) {
			setError(String(e));
		}
	}, []);

	useEffect(() => {
		invoke<string>("get_home_dir").then((home) => {
			setCurrentPath(home);
			loadDirectory(home);
		});
	}, [loadDirectory]);

	const handleNavigate = (entry: FileEntry) => {
		if (entry.is_dir) {
			setCurrentPath(entry.path);
			loadDirectory(entry.path);
			setSelectedPath(null);
		}
	};

	const handleSelect = (entry: FileEntry) => {
		if (entry.is_dir) {
			return;
		}
		if (!entry.is_media) {
			return;
		}
		setSelectedPath(entry.path);
		onFileSelect(entry.path);
	};

	const handleGoUp = () => {
		const parent = currentPath.split("/").slice(0, -1).join("/") || "/";
		setCurrentPath(parent);
		loadDirectory(parent);
		setSelectedPath(null);
	};

	const formatSize = (bytes: number): string => {
		if (bytes === 0) return "-";
		const units = ["B", "KB", "MB", "GB"];
		const i = Math.floor(Math.log(bytes) / Math.log(1024));
		return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
	};

	return (
		<div className="flex flex-col h-full border-r border-border">
			<div className="flex items-center gap-2 px-3 py-2 border-b border-border bg-card">
				<Button
					variant="ghost"
					size="icon"
					onClick={handleGoUp}
					disabled={currentPath === "/"}
				>
					<ChevronLeft className="h-4 w-4" />
				</Button>
				<span className="text-xs text-muted-foreground truncate flex-1">
					{currentPath}
				</span>
			</div>

			{error && (
				<div className="px-3 py-2 bg-destructive/10 text-destructive text-xs">
					{error}
				</div>
			)}

			<ScrollArea className="flex-1 h-[calc(100%-4rem)]">
				<div className="p-1">
					{files.map((file) => (
						<button
							type="button"
							key={file.path}
							className={cn(
								"grid w-full grid-cols-[20px_1fr_70px_130px] gap-2 items-center px-2 py-1.5 rounded-md cursor-pointer text-sm transition-colors",
								"hover:bg-accent",
								selectedPath === file.path && "bg-accent",
								!file.is_dir && !file.is_media && "opacity-40 hover:opacity-60",
								file.is_dir && "font-medium",
							)}
							onClick={() => handleSelect(file)}
							onDoubleClick={() => handleNavigate(file)}
						>
							<span className="text-muted-foreground">
								{file.is_dir ? (
									<Folder className="h-4 w-4" />
								) : file.is_media ? (
									<Film className="h-4 w-4" />
								) : (
									<FileText className="h-4 w-4" />
								)}
							</span>
							<span className="truncate text-left">{file.name}</span>
							<span className="text-xs text-muted-foreground text-right">
								{file.is_dir ? "-" : formatSize(file.size)}
							</span>
							<span className="text-xs text-muted-foreground text-right">
								{file.modified || "-"}
							</span>
						</button>
					))}
				</div>
			</ScrollArea>
		</div>
	);
}
