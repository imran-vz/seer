import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, ExternalLink, RefreshCw, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "./ui/scroll-area";

interface DependencyStatus {
	name: string;
	installed: boolean;
	version: string | null;
}

interface DependenciesResult {
	all_installed: boolean;
	dependencies: DependencyStatus[];
	platform: string;
}

const installGuides: Record<
	string,
	Record<string, { cmd: string; url: string }>
> = {
	ffmpeg: {
		macos: {
			cmd: "brew install ffmpeg",
			url: "https://formulae.brew.sh/formula/ffmpeg",
		},
		windows: {
			cmd: "winget install FFmpeg.FFmpeg",
			url: "https://www.ffmpeg.org/download.html#build-windows",
		},
		linux: {
			cmd: "sudo apt install ffmpeg",
			url: "https://ffmpeg.org/download.html#build-linux",
		},
	},
	ffprobe: {
		macos: {
			cmd: "brew install ffmpeg",
			url: "https://formulae.brew.sh/formula/ffmpeg",
		},
		windows: {
			cmd: "winget install FFmpeg.FFmpeg",
			url: "https://www.ffmpeg.org/download.html#build-windows",
		},
		linux: {
			cmd: "sudo apt install ffmpeg",
			url: "https://ffmpeg.org/download.html#build-linux",
		},
	},
	exiftool: {
		macos: {
			cmd: "brew install exiftool",
			url: "https://formulae.brew.sh/formula/exiftool",
		},
		windows: {
			cmd: "winget install OliverBetz.ExifTool",
			url: "https://exiftool.org/install.html#Windows",
		},
		linux: {
			cmd: "sudo apt install libimage-exiftool-perl",
			url: "https://exiftool.org/install.html#Unix",
		},
	},
};

interface DependencyCheckProps {
	onComplete: () => void;
}

export function DependencyCheck({ onComplete }: DependencyCheckProps) {
	const [result, setResult] = useState<DependenciesResult | null>(null);
	const [loading, setLoading] = useState(true);

	const checkDeps = useCallback(async () => {
		setLoading(true);
		try {
			const data = await invoke<DependenciesResult>("check_dependencies");
			setResult(data);
			if (data.all_installed) {
				onComplete();
			}
		} catch (e) {
			console.error("Failed to check dependencies:", e);
		} finally {
			setLoading(false);
		}
	}, [onComplete]);

	useEffect(() => {
		checkDeps();
	}, [checkDeps]);

	if (loading && !result) {
		return (
			<div className="flex items-center justify-center h-screen bg-background">
				<p className="text-muted-foreground">Checking dependencies...</p>
			</div>
		);
	}

	if (!result || result.all_installed) {
		return null;
	}

	const missingDeps = result.dependencies.filter((d) => !d.installed);

	return (
		<div className="flex flex-col items-center justify-center bg-background p-8">
			<ScrollArea className="h-[calc(100vh-6rem)]!">
				<div className="max-w-lg w-full">
					<div className="text-center mb-8">
						<h1 className="text-2xl font-bold mb-2">Setup Required</h1>
						<p className="text-muted-foreground">
							Seer requires the following tools to be installed on your system.
						</p>
					</div>

					<div className="flex justify-center mb-8">
						<Button onClick={checkDeps} disabled={loading}>
							<RefreshCw
								className={`w-4 h-4 mr-2 ${loading ? "animate-spin" : ""}`}
							/>
							{loading ? "Checking..." : "Check Again"}
						</Button>
					</div>

					<div className="space-y-4 mb-8">
						{result.dependencies.map((dep) => {
							const guide = installGuides[dep.name]?.[result.platform];
							return (
								<div
									key={dep.name}
									className="flex items-center justify-between p-4 rounded-lg border border-border bg-card"
								>
									<div className="flex items-center gap-3">
										{dep.installed ? (
											<Check className="w-5 h-5 text-green-500" />
										) : (
											<X className="w-5 h-5 text-destructive" />
										)}
										<div>
											<p className="font-medium">{dep.name}</p>
											{dep.installed && dep.version && (
												<p className="text-xs text-muted-foreground truncate max-w-50">
													{dep.version}
												</p>
											)}
										</div>
									</div>
									{!dep.installed && guide && (
										<Button
											variant="outline"
											size="sm"
											onClick={() => openUrl(guide.url)}
										>
											<ExternalLink className="w-4 h-4 mr-1" />
											Install
										</Button>
									)}
								</div>
							);
						})}
					</div>

					{missingDeps.length > 0 && (
						<div className="p-4 rounded-lg border border-border bg-card mb-6">
							<h3 className="font-medium mb-3">Installation Commands</h3>
							<p className="text-xs text-muted-foreground mb-3">
								Run these commands in your terminal:
							</p>
							<div className="space-y-2">
								{[
									...new Set(
										missingDeps.map(
											(d) => installGuides[d.name]?.[result.platform]?.cmd,
										),
									),
								]
									.filter(Boolean)
									.map((cmd) => (
										<code
											key={cmd}
											className="block p-2 rounded bg-muted text-sm font-mono"
										>
											{cmd}
										</code>
									))}
							</div>
						</div>
					)}
				</div>
			</ScrollArea>
		</div>
	);
}
