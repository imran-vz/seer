import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
	Check,
	Download,
	ExternalLink,
	Loader2,
	RefreshCw,
	X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { useInstallerStore } from "@/stores/installerStore";
import { Progress } from "./ui/progress";
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

interface DependencyCheckProps {
	onComplete: () => void;
}

export function DependencyCheck({ onComplete }: DependencyCheckProps) {
	const [result, setResult] = useState<DependenciesResult | null>(null);
	const [loading, setLoading] = useState(true);

	const installerStore = useInstallerStore();

	const checkDeps = useCallback(async () => {
		setLoading(true);
		try {
			const data = await invoke<DependenciesResult>("check_dependencies");
			setResult(data);
			if (data.all_installed) {
				onComplete();
			} else {
				// Load install strategies for missing dependencies
				const missing = data.dependencies.filter((d) => !d.installed);
				for (const dep of missing) {
					// Map ffprobe to ffmpeg (same package)
					const toolName = dep.name === "ffprobe" ? "ffmpeg" : dep.name;
					await installerStore.getStrategies(toolName);
				}
			}
		} catch (e) {
			console.error("Failed to check dependencies:", e);
		} finally {
			setLoading(false);
		}
	}, [installerStore, onComplete]);

	useEffect(() => {
		checkDeps();
	}, [checkDeps]);

	const handleInstall = async (depName: string) => {
		// Map ffprobe to ffmpeg (same package)
		const toolName = depName === "ffprobe" ? "ffmpeg" : depName;
		await installerStore.install(toolName);

		// Recheck dependencies after installation
		setTimeout(() => {
			checkDeps();
		}, 1000);
	};

	if (loading && !result) {
		return (
			<div className="flex h-screen items-center justify-center bg-background">
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
			<ScrollArea className="h-[calc(100vh-6rem)]">
				<div className="w-full max-w-2xl">
					<div className="mb-8 text-center">
						<h1 className="mb-2 font-bold text-2xl">Setup Required</h1>
						<p className="text-muted-foreground">
							Seer requires FFmpeg to process media files. Let's install it
							automatically.
						</p>
					</div>

					<div className="mb-8 flex justify-center">
						<Button onClick={checkDeps} disabled={loading}>
							<RefreshCw
								className={`mr-2 h-4 w-4 ${loading ? "animate-spin" : ""}`}
							/>
							{loading ? "Checking..." : "Check Again"}
						</Button>
					</div>

					<div className="mb-8 space-y-4">
						{result.dependencies.map((dep) => {
							// Map ffprobe to ffmpeg for installer
							const toolName = dep.name === "ffprobe" ? "ffmpeg" : dep.name;
							const toolState = installerStore.tools[toolName];
							const isInstalling = toolState?.status === "installing";
							const strategies = toolState?.strategies ?? [];
							const hasStrategies = strategies.length > 0;

							return (
								<div
									key={dep.name}
									className="rounded-lg border border-border bg-card p-4"
								>
									<div className="flex items-start justify-between">
										<div className="flex items-center gap-3">
											{dep.installed ? (
												<Check className="mt-1 h-5 w-5 text-green-500" />
											) : isInstalling ? (
												<Loader2 className="mt-1 h-5 w-5 animate-spin text-primary" />
											) : (
												<X className="mt-1 h-5 w-5 text-destructive" />
											)}
											<div className="flex-1">
												<p className="font-medium">{dep.name}</p>
												{dep.installed && dep.version && (
													<p className="max-w-96 truncate text-muted-foreground text-xs">
														{dep.version}
													</p>
												)}

												{/* Installation progress */}
												{isInstalling && toolState.progress && (
													<div className="mt-3 space-y-2">
														<div className="flex items-center justify-between text-xs">
															<span className="text-muted-foreground">
																{toolState.progress.stage}
															</span>
															<span className="text-muted-foreground">
																{Math.round(toolState.progress.percentage)}%
															</span>
														</div>
														<Progress value={toolState.progress.percentage} />
													</div>
												)}

												{/* Installation result */}
												{toolState?.status === "success" &&
													toolState.result && (
														<p className="mt-2 text-green-600 text-xs">
															{toolState.result.message}
														</p>
													)}
												{toolState?.status === "failed" && toolState.result && (
													<p className="mt-2 text-destructive text-xs">
														{toolState.result.message}
													</p>
												)}
											</div>
										</div>

										{!dep.installed && !isInstalling && (
											<div className="flex flex-col gap-2">
												{/* Method selector */}
												{hasStrategies && strategies.length > 1 && (
													<select
														value={toolState.selectedMethod}
														onChange={(e) =>
															installerStore.setSelectedMethod(
																toolName,
																e.target.value,
															)
														}
														className="h-9 w-40 rounded-md border border-input bg-background px-3 text-xs ring-offset-background focus:outline-none focus:ring-2 focus:ring-ring focus:ring-offset-2"
													>
														{strategies.map((strategy) => (
															<option
																key={strategy.method}
																value={strategy.method}
															>
																{strategy.method}
																{strategy.requires_admin && " (sudo)"}
															</option>
														))}
													</select>
												)}

												{/* Install button */}
												<Button
													size="sm"
													onClick={() => handleInstall(dep.name)}
													disabled={!hasStrategies}
												>
													<Download className="mr-1 h-4 w-4" />
													Install
												</Button>

												{/* Manual link */}
												<Button
													variant="ghost"
													size="sm"
													onClick={() =>
														openUrl("https://ffmpeg.org/download.html")
													}
												>
													<ExternalLink className="mr-1 h-3 w-3" />
													Manual
												</Button>
											</div>
										)}
									</div>
								</div>
							);
						})}
					</div>

					{/* Info box */}
					{missingDeps.length > 0 && (
						<div className="rounded-lg border border-border bg-card p-4">
							<h3 className="mb-2 font-medium text-sm">
								What will be installed?
							</h3>
							<ul className="space-y-1 text-muted-foreground text-xs">
								<li>• FFmpeg - Media processing toolkit (~80MB)</li>
								<li>
									• Installed to:{" "}
									{installerStore.tools.ffmpeg?.strategies?.[0]
										?.install_location || "system location"}
								</li>
								<li>
									• Method:{" "}
									{installerStore.tools.ffmpeg?.selectedMethod ||
										"Auto-detected"}
								</li>
							</ul>
						</div>
					)}
				</div>
			</ScrollArea>
		</div>
	);
}
