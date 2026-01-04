import {
	FolderOpen,
	HardDrive,
	Info,
	RefreshCw,
	RotateCcw,
	Settings,
	Trash2,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import {
	clearAllBitrateAnalysis,
	clearAllCache,
	clearAllJobs,
	getBitrateAnalysisStats,
	getCacheStatistics,
	getJobStatistics,
	runMaintenance,
} from "@/lib/database";
import { useSettingsStore } from "@/stores/settingsStore";
import type { CacheStatistics, JobStatistics } from "@/types/database";
import {
	AlertDialog,
	AlertDialogAction,
	AlertDialogCancel,
	AlertDialogContent,
	AlertDialogDescription,
	AlertDialogFooter,
	AlertDialogHeader,
	AlertDialogTitle,
} from "./ui/alert-dialog";
import { Button } from "./ui/button";
import { Checkbox } from "./ui/checkbox";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "./ui/dialog";
import { Input } from "./ui/input";

interface BitrateAnalysisStats {
	total_analyses: number;
	total_data_points: number;
	oldest_analysis: string | null;
	newest_analysis: string | null;
}

function SettingsSection({
	title,
	description,
	children,
}: {
	title: string;
	description?: string;
	children: React.ReactNode;
}) {
	return (
		<div className="rounded-lg border border-border bg-card p-4">
			<h3 className="mb-1 font-semibold text-sm">{title}</h3>
			{description && (
				<p className="mb-3 text-muted-foreground text-xs">{description}</p>
			)}
			<div className="space-y-3">{children}</div>
		</div>
	);
}

function SettingsRow({
	label,
	description,
	children,
}: {
	label: string;
	description?: string;
	children: React.ReactNode;
}) {
	return (
		<div className="flex items-center justify-between gap-4">
			<div className="min-w-0 flex-1">
				<p className="text-sm">{label}</p>
				{description && (
					<p className="text-muted-foreground text-xs">{description}</p>
				)}
			</div>
			<div className="shrink-0">{children}</div>
		</div>
	);
}

function SettingsToggle({
	label,
	description,
	checked,
	onCheckedChange,
	disabled,
}: {
	label: string;
	description?: string;
	checked: boolean;
	onCheckedChange: (checked: boolean) => void;
	disabled?: boolean;
}) {
	return (
		<div className="flex items-center justify-between gap-4">
			<div className="min-w-0 flex-1">
				<p className="text-sm">{label}</p>
				{description && (
					<p className="text-muted-foreground text-xs">{description}</p>
				)}
			</div>
			<Checkbox
				checked={checked}
				onCheckedChange={onCheckedChange}
				disabled={disabled}
			/>
		</div>
	);
}

function StatItem({ label, value }: { label: string; value: string | number }) {
	return (
		<div className="flex justify-between text-xs">
			<span className="text-muted-foreground">{label}</span>
			<span className="font-medium">{value}</span>
		</div>
	);
}

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const units = ["B", "KB", "MB", "GB"];
	const i = Math.floor(Math.log(bytes) / Math.log(1024));
	return `${(bytes / 1024 ** i).toFixed(2)} ${units[i]}`;
}

function formatDate(dateStr: string | null): string {
	if (!dateStr) return "-";
	try {
		const date = new Date(dateStr);
		return date.toLocaleDateString(undefined, {
			year: "numeric",
			month: "short",
			day: "numeric",
			hour: "2-digit",
			minute: "2-digit",
		});
	} catch {
		return dateStr;
	}
}

export function SettingsDialog() {
	const [isOpen, setIsOpen] = useState(false);
	const [cacheStats, setCacheStats] = useState<CacheStatistics | null>(null);
	const [bitrateStats, setBitrateStats] = useState<BitrateAnalysisStats | null>(
		null,
	);
	const [jobStats, setJobStats] = useState<JobStatistics | null>(null);
	const [loading, setLoading] = useState(false);
	const [clearCacheDialogOpen, setClearCacheDialogOpen] = useState(false);
	const [clearBitrateDialogOpen, setClearBitrateDialogOpen] = useState(false);
	const [clearJobsDialogOpen, setClearJobsDialogOpen] = useState(false);
	const [clearAllDialogOpen, setClearAllDialogOpen] = useState(false);
	const [resetSettingsDialogOpen, setResetSettingsDialogOpen] = useState(false);

	// Download location state
	const [downloadLocationInput, setDownloadLocationInput] = useState("");
	const [downloadLocationError, setDownloadLocationError] = useState<
		string | null
	>(null);
	const [savingDownloadLocation, setSavingDownloadLocation] = useState(false);

	// Settings store
	const {
		settings,
		loading: settingsLoading,
		initialize: initializeSettings,
		updateSetting,
		resetSettings,
		validatePath,
		pickFolder,
		getDefaultDownloadsDir,
	} = useSettingsStore();

	const loadStats = useCallback(async () => {
		setLoading(true);
		try {
			const [cache, bitrate, jobs] = await Promise.all([
				getCacheStatistics(),
				getBitrateAnalysisStats(),
				getJobStatistics(),
			]);
			setCacheStats(cache);
			setBitrateStats(bitrate);
			setJobStats(jobs);
		} catch (error) {
			console.error("[Settings] Failed to load stats:", error);
			toast.error("Failed to load statistics");
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		if (isOpen) {
			loadStats();
			initializeSettings();
		}
	}, [isOpen, loadStats, initializeSettings]);

	// Sync download location input with settings
	useEffect(() => {
		setDownloadLocationInput(settings.defaultDownloadLocation);
		setDownloadLocationError(null);
	}, [settings.defaultDownloadLocation]);

	const handleClearCache = async () => {
		try {
			const count = await clearAllCache();
			toast.success(`Cleared ${count} cache entries`);
			await loadStats();
		} catch (error) {
			console.error("[Settings] Failed to clear cache:", error);
			toast.error("Failed to clear cache");
		}
		setClearCacheDialogOpen(false);
	};

	const handleClearBitrateAnalysis = async () => {
		try {
			const count = await clearAllBitrateAnalysis();
			toast.success(`Cleared ${count} bitrate analyses`);
			await loadStats();
		} catch (error) {
			console.error("[Settings] Failed to clear bitrate analysis:", error);
			toast.error("Failed to clear bitrate analysis");
		}
		setClearBitrateDialogOpen(false);
	};

	const handleClearJobs = async () => {
		try {
			const count = await clearAllJobs();
			toast.success(`Cleared ${count} job records`);
			await loadStats();
		} catch (error) {
			console.error("[Settings] Failed to clear jobs:", error);
			toast.error("Failed to clear job history");
		}
		setClearJobsDialogOpen(false);
	};

	const handleClearAll = async () => {
		try {
			const [cacheCount, bitrateCount, jobsCount] = await Promise.all([
				clearAllCache(),
				clearAllBitrateAnalysis(),
				clearAllJobs(),
			]);
			toast.success(
				`Cleared ${cacheCount} cache entries, ${bitrateCount} bitrate analyses, and ${jobsCount} jobs`,
			);
			await loadStats();
		} catch (error) {
			console.error("[Settings] Failed to clear all data:", error);
			toast.error("Failed to clear all data");
		}
		setClearAllDialogOpen(false);
	};

	const handleRunMaintenance = async () => {
		try {
			const result = await runMaintenance();
			toast.success(
				`Maintenance complete: ${result.jobsCleaned} jobs, ${result.cacheCleaned} cache entries cleaned`,
			);
			await loadStats();
		} catch (error) {
			console.error("[Settings] Failed to run maintenance:", error);
			toast.error("Failed to run maintenance");
		}
	};

	const handleResetSettings = async () => {
		try {
			await resetSettings();
			toast.success("Settings reset to defaults");
		} catch (error) {
			console.error("[Settings] Failed to reset settings:", error);
			toast.error("Failed to reset settings");
		}
		setResetSettingsDialogOpen(false);
	};

	const handleToggleSetting = async (
		key: keyof typeof settings,
		value: number | boolean,
	) => {
		try {
			await updateSetting(key, value);
		} catch (error) {
			console.error(`[Settings] Failed to update ${key}:`, error);
			toast.error(`Failed to update setting`);
		}
	};

	const handlePickDownloadFolder = async () => {
		const currentPath =
			downloadLocationInput || (await getDefaultDownloadsDir());
		const selectedPath = await pickFolder(
			"Select Download Location",
			currentPath,
		);

		if (selectedPath) {
			setDownloadLocationInput(selectedPath);
			setDownloadLocationError(null);
			await saveDownloadLocation(selectedPath);
		}
	};

	const saveDownloadLocation = async (path: string) => {
		if (!path.trim()) {
			// Empty path is valid (will use system default)
			try {
				await updateSetting("defaultDownloadLocation", "");
				setDownloadLocationError(null);
				toast.success("Download location cleared (will use system default)");
			} catch (error) {
				console.error("[Settings] Failed to save download location:", error);
				toast.error("Failed to save download location");
			}
			return;
		}

		setSavingDownloadLocation(true);
		setDownloadLocationError(null);

		try {
			// Validate and create directory if needed
			const result = await validatePath(path, true);

			if (!result.valid) {
				setDownloadLocationError(result.error || "Invalid path");
				toast.error(result.error || "Invalid path");
				return;
			}

			// Save to database
			await updateSetting("defaultDownloadLocation", result.path);

			if (result.created) {
				toast.success(`Created folder and saved: ${result.path}`);
			} else {
				toast.success("Download location saved");
			}
		} catch (error) {
			const errorMsg =
				error instanceof Error ? error.message : "Failed to save";
			setDownloadLocationError(errorMsg);
			toast.error(errorMsg);
		} finally {
			setSavingDownloadLocation(false);
		}
	};

	const handleDownloadLocationBlur = async () => {
		// Only save if the value has changed
		if (downloadLocationInput !== settings.defaultDownloadLocation) {
			await saveDownloadLocation(downloadLocationInput);
		}
	};

	const handleDownloadLocationKeyDown = async (
		e: React.KeyboardEvent<HTMLInputElement>,
	) => {
		if (e.key === "Enter") {
			e.preventDefault();
			await saveDownloadLocation(downloadLocationInput);
		}
	};

	return (
		<>
			<Dialog open={isOpen} onOpenChange={setIsOpen}>
				<DialogTrigger asChild>
					<Button variant="ghost" size="icon-sm" title="Settings">
						<Settings className="size-4" />
					</Button>
				</DialogTrigger>
				<DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-125">
					<DialogHeader>
						<DialogTitle className="flex items-center gap-2">
							<Settings className="size-5" />
							Settings
						</DialogTitle>
						<DialogDescription>
							Manage application preferences, cache, and data storage.
						</DialogDescription>
					</DialogHeader>

					<div className="space-y-4 py-2">
						{/* Storage & Cache Section */}
						<SettingsSection
							title="Storage & Cache"
							description="Manage cached data and database storage"
						>
							{/* Cache Statistics */}
							{cacheStats && (
								<div className="rounded-md bg-muted/50 p-3">
									<div className="mb-2 flex items-center gap-2">
										<HardDrive className="size-4 text-muted-foreground" />
										<span className="font-medium text-sm">
											Cache Statistics
										</span>
									</div>
									<div className="space-y-1">
										<StatItem
											label="Total Entries"
											value={cacheStats.total_entries}
										/>
										<StatItem
											label="Storage Used"
											value={formatBytes(cacheStats.total_size_bytes)}
										/>
										<StatItem
											label="Newest Entry"
											value={formatDate(cacheStats.newest_entry)}
										/>
									</div>
								</div>
							)}

							{/* Bitrate Analysis Statistics */}
							{bitrateStats && (
								<div className="rounded-md bg-muted/50 p-3">
									<div className="mb-2 flex items-center gap-2">
										<Info className="size-4 text-muted-foreground" />
										<span className="font-medium text-sm">
											Bitrate Analysis Data
										</span>
									</div>
									<div className="space-y-1">
										<StatItem
											label="Total Analyses"
											value={bitrateStats.total_analyses}
										/>
										<StatItem
											label="Data Points"
											value={bitrateStats.total_data_points.toLocaleString()}
										/>
										<StatItem
											label="Newest Analysis"
											value={formatDate(bitrateStats.newest_analysis)}
										/>
									</div>
								</div>
							)}

							{/* Job Statistics */}
							{jobStats && (
								<div className="rounded-md bg-muted/50 p-3">
									<div className="mb-2 flex items-center gap-2">
										<RefreshCw className="size-4 text-muted-foreground" />
										<span className="font-medium text-sm">Job History</span>
									</div>
									<div className="space-y-1">
										<StatItem label="Total Jobs" value={jobStats.total_jobs} />
										<StatItem
											label="Completed"
											value={jobStats.completed_jobs}
										/>
										<StatItem label="Failed" value={jobStats.failed_jobs} />
									</div>
								</div>
							)}

							{/* Refresh Stats */}
							<SettingsRow
								label="Refresh Statistics"
								description="Reload cache and database statistics"
							>
								<Button
									variant="outline"
									size="sm"
									onClick={loadStats}
									disabled={loading}
								>
									<RefreshCw
										className={`mr-1 size-3 ${loading ? "animate-spin" : ""}`}
									/>
									Refresh
								</Button>
							</SettingsRow>

							{/* Run Maintenance */}
							<SettingsRow
								label="Run Maintenance"
								description="Clean up expired cache and old jobs"
							>
								<Button
									variant="outline"
									size="sm"
									onClick={handleRunMaintenance}
								>
									<RefreshCw className="mr-1 size-3" />
									Run
								</Button>
							</SettingsRow>

							{/* Clear Cache */}
							<SettingsRow
								label="Clear Cache"
								description="Remove all cached metadata and ffprobe data"
							>
								<Button
									variant="outline"
									size="sm"
									onClick={() => setClearCacheDialogOpen(true)}
								>
									<Trash2 className="mr-1 size-3" />
									Clear
								</Button>
							</SettingsRow>

							{/* Clear Bitrate Analysis */}
							<SettingsRow
								label="Clear Bitrate Data"
								description="Remove all stored bitrate analysis results"
							>
								<Button
									variant="outline"
									size="sm"
									onClick={() => setClearBitrateDialogOpen(true)}
								>
									<Trash2 className="mr-1 size-3" />
									Clear
								</Button>
							</SettingsRow>

							{/* Clear Job History */}
							<SettingsRow
								label="Clear Job History"
								description="Remove all job records from the database"
							>
								<Button
									variant="outline"
									size="sm"
									onClick={() => setClearJobsDialogOpen(true)}
								>
									<Trash2 className="mr-1 size-3" />
									Clear
								</Button>
							</SettingsRow>

							{/* Clear All Data */}
							<SettingsRow
								label="Clear All Data"
								description="Remove all cached data, analysis results, and job history"
							>
								<Button
									variant="destructive"
									size="sm"
									onClick={() => setClearAllDialogOpen(true)}
								>
									<Trash2 className="mr-1 size-3" />
									Clear All
								</Button>
							</SettingsRow>
						</SettingsSection>

						{/* File Browser Preferences */}
						<SettingsSection
							title="File Browser"
							description="Customize file browser behavior"
						>
							<SettingsToggle
								label="Show Hidden Files"
								description="Display files and folders starting with a dot"
								checked={settings.showHiddenFiles}
								onCheckedChange={(checked) =>
									handleToggleSetting("showHiddenFiles", checked)
								}
								disabled={settingsLoading}
							/>

							<SettingsToggle
								label="Remember Last Directory"
								description="Start in the last visited directory when opening the app"
								checked={settings.startInLastDirectory}
								onCheckedChange={(checked) =>
									handleToggleSetting("startInLastDirectory", checked)
								}
								disabled={settingsLoading}
							/>

							<SettingsToggle
								label="Confirm Before Delete"
								description="Show confirmation dialog before deleting files"
								checked={settings.confirmBeforeDelete}
								onCheckedChange={(checked) =>
									handleToggleSetting("confirmBeforeDelete", checked)
								}
								disabled={settingsLoading}
							/>

							<SettingsToggle
								label="Use Trash by Default"
								description="Move files to trash instead of permanent deletion"
								checked={settings.useTrashByDefault}
								onCheckedChange={(checked) =>
									handleToggleSetting("useTrashByDefault", checked)
								}
								disabled={settingsLoading}
							/>
						</SettingsSection>

						{/* Downloads & Export Preferences */}
						<SettingsSection
							title="Downloads & Export"
							description="Configure export and download settings"
						>
							{/* Download Location */}
							<div className="space-y-2">
								<SettingsRow
									label="Default Download Location"
									description="Where to save exported files (leave empty for system default)"
								>
									<div className="flex items-center gap-2">
										<div className="relative">
											<Input
												value={downloadLocationInput}
												onChange={(e) => {
													setDownloadLocationInput(e.target.value);
													setDownloadLocationError(null);
												}}
												onBlur={handleDownloadLocationBlur}
												onKeyDown={handleDownloadLocationKeyDown}
												placeholder="~/Downloads"
												className={`h-8 w-44 pr-8 text-xs ${
													downloadLocationError
														? "border-destructive focus-visible:ring-destructive/50"
														: ""
												}`}
												disabled={savingDownloadLocation}
											/>
											{savingDownloadLocation && (
												<RefreshCw className="absolute top-1/2 right-2 size-3 -translate-y-1/2 animate-spin text-muted-foreground" />
											)}
										</div>
										<Button
											variant="outline"
											size="icon-sm"
											title="Browse for folder"
											onClick={handlePickDownloadFolder}
											disabled={savingDownloadLocation}
										>
											<FolderOpen className="size-3" />
										</Button>
									</div>
								</SettingsRow>
								{downloadLocationError && (
									<p className="text-destructive text-xs">
										{downloadLocationError}
									</p>
								)}
							</div>
						</SettingsSection>

						{/* Performance Settings */}
						<SettingsSection
							title="Performance"
							description="Configure analysis performance settings"
						>
							<SettingsRow
								label="Max Parallel Jobs"
								description="Number of bitrate analyses to run simultaneously (1-8)"
							>
								<Input
									type="number"
									min={1}
									max={8}
									value={settings.maxParallelJobs}
									onChange={(e) => {
										const value = Number.parseInt(e.target.value, 10);
										if (!Number.isNaN(value) && value >= 1 && value <= 8) {
											handleToggleSetting("maxParallelJobs", value);
										}
									}}
									className="h-8 w-16 text-center text-xs"
									disabled={settingsLoading}
								/>
							</SettingsRow>
						</SettingsSection>

						{/* Reset Settings */}
						<SettingsSection title="Reset">
							<SettingsRow
								label="Reset All Settings"
								description="Restore all settings to their default values"
							>
								<Button
									variant="outline"
									size="sm"
									onClick={() => setResetSettingsDialogOpen(true)}
								>
									<RotateCcw className="mr-1 size-3" />
									Reset
								</Button>
							</SettingsRow>
						</SettingsSection>

						<SettingsSection title="Developer">
							<div className="space-y-2 text-muted-foreground text-xs">
								<p>
									<strong className="text-foreground">Imran</strong> - Developer
								</p>
								<p>
									<strong className="text-foreground">GitHub</strong> -{" "}
									<a
										href="https://github.com/imran-vz"
										className="text-primary"
										target="_blank"
										rel="noopener"
									>
										https://github.com/imran-vz
									</a>
								</p>
								<p>
									<strong className="text-foreground">Website</strong> -{" "}
									<a
										href="https://imran.codes"
										className="text-primary"
										target="_blank"
										rel="noopener"
									>
										https://imran.codes
									</a>
								</p>
							</div>
						</SettingsSection>
						{/* About Section */}
						<SettingsSection title="About">
							<div className="space-y-2 text-muted-foreground text-xs">
								<p>
									<strong className="text-foreground">Seer</strong> - Media file
									metadata editor and analyzer
								</p>
								<p>Version 0.1.0</p>
								<p className="text-muted-foreground/80">
									The name "Seer" comes from Tamil, meaning to prune, order,
									uniformity, and neatness.
								</p>
							</div>
						</SettingsSection>
					</div>
				</DialogContent>
			</Dialog>

			{/* Clear Cache Confirmation */}
			<AlertDialog
				open={clearCacheDialogOpen}
				onOpenChange={setClearCacheDialogOpen}
			>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Clear Cache?</AlertDialogTitle>
						<AlertDialogDescription>
							This will remove all cached metadata and ffprobe data. The data
							will be regenerated when you view files again.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction onClick={handleClearCache}>
							Clear Cache
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>

			{/* Clear Bitrate Analysis Confirmation */}
			<AlertDialog
				open={clearBitrateDialogOpen}
				onOpenChange={setClearBitrateDialogOpen}
			>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Clear Bitrate Analysis Data?</AlertDialogTitle>
						<AlertDialogDescription>
							This will remove all stored bitrate analysis results. You'll need
							to re-analyze files to see their bitrate graphs.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction onClick={handleClearBitrateAnalysis}>
							Clear Data
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>

			{/* Clear Jobs Confirmation */}
			<AlertDialog
				open={clearJobsDialogOpen}
				onOpenChange={setClearJobsDialogOpen}
			>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Clear Job History?</AlertDialogTitle>
						<AlertDialogDescription>
							This will remove all job records from the database. This is useful
							to free up space but will remove the history of all processed
							files.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction onClick={handleClearJobs}>
							Clear Jobs
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>

			{/* Clear All Data Confirmation */}
			<AlertDialog
				open={clearAllDialogOpen}
				onOpenChange={setClearAllDialogOpen}
			>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Clear All Data?</AlertDialogTitle>
						<AlertDialogDescription>
							This will permanently delete all cached data, bitrate analyses,
							and job history. This action cannot be undone.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction
							className="bg-destructive text-white hover:bg-destructive/90"
							onClick={handleClearAll}
						>
							Clear All Data
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>

			{/* Reset Settings Confirmation */}
			<AlertDialog
				open={resetSettingsDialogOpen}
				onOpenChange={setResetSettingsDialogOpen}
			>
				<AlertDialogContent>
					<AlertDialogHeader>
						<AlertDialogTitle>Reset All Settings?</AlertDialogTitle>
						<AlertDialogDescription>
							This will restore all settings to their default values. Your
							cached data and analysis results will not be affected.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel>Cancel</AlertDialogCancel>
						<AlertDialogAction onClick={handleResetSettings}>
							Reset Settings
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	);
}
