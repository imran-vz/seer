import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { useSettingsStore } from "./settingsStore";

export interface FileEntry {
	name: string;
	path: string;
	is_dir: boolean;
	is_media: boolean;
	size: number;
	modified: string | null;
}

interface FileOperationResult {
	success: boolean;
	message: string;
	new_path: string | null;
}

interface FileBrowserState {
	currentPath: string;
	files: FileEntry[];
	selectedPath: string | null;
	selectedPaths: Set<string>;
	error: string | null;
	loading: boolean;
	initialized: boolean;
	initializing: boolean;

	// Actions
	setCurrentPath: (path: string) => void;
	setSelectedPath: (path: string | null) => void;
	loadDirectory: (path: string, saveToSettings?: boolean) => Promise<void>;
	refresh: () => Promise<void>;
	goUp: () => Promise<void>;
	navigate: (entry: FileEntry) => Promise<void>;
	selectFile: (entry: FileEntry) => void;
	navigateToFile: (filePath: string) => Promise<void>;
	initialize: () => Promise<void>;

	// Bulk selection
	toggleSelection: (path: string) => void;
	selectAll: () => void;
	clearSelection: () => void;
	isSelected: (path: string) => boolean;

	// File operations
	renameFile: (path: string, newName: string) => Promise<FileOperationResult>;
	deleteFile: (
		path: string,
		permanent: boolean,
	) => Promise<FileOperationResult>;
	moveFile: (path: string, destination: string) => Promise<FileOperationResult>;
	copyFile: (path: string, destination: string) => Promise<FileOperationResult>;
	createFolder: (name: string) => Promise<FileOperationResult>;
	revealInFolder: (path: string) => Promise<FileOperationResult>;

	// Bulk operations
	deleteSelected: (
		permanent: boolean,
	) => Promise<{ success: number; failed: number }>;
	moveSelected: (
		destination: string,
	) => Promise<{ success: number; failed: number }>;
	copySelected: (
		destination: string,
	) => Promise<{ success: number; failed: number }>;
}

export const useFileBrowserStore = create<FileBrowserState>((set, get) => ({
	currentPath: "",
	files: [],
	selectedPath: null,
	selectedPaths: new Set<string>(),
	error: null,
	loading: false,
	initialized: false,
	initializing: false,

	setCurrentPath: (path) => set({ currentPath: path }),

	setSelectedPath: (path) => set({ selectedPath: path }),

	loadDirectory: async (path: string, saveToSettings = true) => {
		set({ loading: true, error: null, selectedPaths: new Set() });
		try {
			const entries = await invoke<FileEntry[]>("list_directory", { path });
			set({ files: entries, currentPath: path, loading: false });

			// Save last directory to settings if enabled
			if (saveToSettings) {
				const settingsStore = useSettingsStore.getState();
				console.log(
					"[FileBrowser] Checking if should save last directory:",
					path,
					{
						startInLastDirectory: settingsStore.settings.startInLastDirectory,
					},
				);
				if (settingsStore.settings.startInLastDirectory) {
					console.log("[FileBrowser] Saving last directory:", path);
					// Don't await this - let it happen in the background
					settingsStore.setLastDirectory(path).catch((err) => {
						console.warn("[FileBrowser] Failed to save last directory:", err);
					});
				} else {
					console.log(
						"[FileBrowser] Not saving - startInLastDirectory is false",
					);
				}
			}
		} catch (e) {
			set({ error: String(e), loading: false });
		}
	},

	refresh: async () => {
		const { currentPath, loadDirectory } = get();
		await loadDirectory(currentPath, false); // Don't save on refresh
	},

	goUp: async () => {
		const { currentPath, loadDirectory } = get();
		const parent = currentPath.split("/").slice(0, -1).join("/") || "/";
		set({ selectedPath: null, selectedPaths: new Set() });
		await loadDirectory(parent);
	},

	navigate: async (entry: FileEntry) => {
		if (entry.is_dir) {
			const { loadDirectory } = get();
			set({ selectedPath: null, selectedPaths: new Set() });
			await loadDirectory(entry.path);
		}
	},

	selectFile: (entry: FileEntry) => {
		if (!entry.is_dir && entry.is_media) {
			set({ selectedPath: entry.path });
		}
	},

	navigateToFile: async (filePath: string) => {
		const { currentPath, loadDirectory } = get();
		// Extract the parent directory from the file path
		const lastSlashIndex = filePath.lastIndexOf("/");
		if (lastSlashIndex === -1) return;

		const parentDir = filePath.substring(0, lastSlashIndex) || "/";

		// If we're not already in the parent directory, navigate to it
		if (currentPath !== parentDir) {
			await loadDirectory(parentDir);
		}

		// Select the file
		set({ selectedPath: filePath });
	},

	initialize: async () => {
		// Prevent double initialization - check both flags
		const state = get();
		if (state.initialized || state.initializing) return;

		// Set initializing flag immediately to prevent race conditions
		set({ initializing: true });

		const { loadDirectory } = get();

		try {
			// Initialize settings store first
			const settingsStore = useSettingsStore.getState();
			await settingsStore.initialize();

			// Re-fetch state after initialization to get updated settings
			const { settings } = useSettingsStore.getState();
			let initialPath: string | null = null;

			console.log("[FileBrowser] Settings loaded:", {
				startInLastDirectory: settings.startInLastDirectory,
				lastDirectory: settings.lastDirectory,
			});

			// Check if we should use the last directory
			if (settings.startInLastDirectory && settings.lastDirectory) {
				console.log(
					"[FileBrowser] Attempting to use last directory:",
					settings.lastDirectory,
				);
				// Validate the last directory still exists
				try {
					const result = await invoke<{ valid: boolean; exists: boolean }>(
						"validate_path",
						{
							path: settings.lastDirectory,
							createIfMissing: false,
						},
					);

					console.log("[FileBrowser] Validation result:", result);

					if (result.valid && result.exists) {
						initialPath = settings.lastDirectory;
						console.log(
							"[FileBrowser] Using last directory:",
							settings.lastDirectory,
						);
					} else {
						console.log(
							"[FileBrowser] Last directory invalid or doesn't exist",
						);
					}
				} catch (err) {
					console.warn("[FileBrowser] Failed to validate last directory:", err);
				}
			} else {
				console.log(
					"[FileBrowser] Not using last directory:",
					!settings.startInLastDirectory
						? "startInLastDirectory is false"
						: "lastDirectory is empty",
				);
			}

			console.log(initialPath);
			// Fall back to home directory if no valid last directory
			if (!initialPath) {
				initialPath = await invoke<string>("get_home_dir");
				console.log("[FileBrowser] Using home directory:", initialPath);
			}

			set({ initialized: true, initializing: false });
			await loadDirectory(initialPath, false); // Don't save initial load
		} catch (e) {
			console.error("[FileBrowser] Initialization error:", e);
			set({ error: String(e), initialized: true, initializing: false });

			// Try to at least load the home directory as fallback
			try {
				const home = await invoke<string>("get_home_dir");
				await loadDirectory(home, false);
			} catch (fallbackError) {
				console.error("[FileBrowser] Fallback also failed:", fallbackError);
			}
		}
	},

	// Bulk selection
	toggleSelection: (path: string) => {
		const { selectedPaths } = get();
		const newSelection = new Set(selectedPaths);
		if (newSelection.has(path)) {
			newSelection.delete(path);
		} else {
			newSelection.add(path);
		}
		set({ selectedPaths: newSelection });
	},

	selectAll: () => {
		const { files } = get();
		const allPaths = new Set(files.map((f) => f.path));
		set({ selectedPaths: allPaths });
	},

	clearSelection: () => {
		set({ selectedPaths: new Set() });
	},

	isSelected: (path: string) => {
		return get().selectedPaths.has(path);
	},

	// File operations
	renameFile: async (path: string, newName: string) => {
		try {
			const result = await invoke<FileOperationResult>("rename_file", {
				path,
				newName,
			});
			const { refresh, selectedPath } = get();
			// Update selected path if the renamed file was selected
			if (selectedPath === path && result.new_path) {
				set({ selectedPath: result.new_path });
			}
			await refresh();
			return result;
		} catch (e) {
			throw new Error(String(e));
		}
	},

	deleteFile: async (path: string, permanent: boolean) => {
		try {
			const result = await invoke<FileOperationResult>("delete_file", {
				path,
				permanent,
			});
			const { refresh, selectedPath } = get();
			// Clear selected path if deleted file was selected
			if (selectedPath === path) {
				set({ selectedPath: null });
			}
			await refresh();
			return result;
		} catch (e) {
			throw new Error(String(e));
		}
	},

	moveFile: async (path: string, destination: string) => {
		try {
			const result = await invoke<FileOperationResult>("move_file", {
				path,
				destination,
			});
			const { refresh, selectedPath } = get();
			// Update selected path if moved file was selected
			if (selectedPath === path && result.new_path) {
				set({ selectedPath: result.new_path });
			}
			await refresh();
			return result;
		} catch (e) {
			throw new Error(String(e));
		}
	},

	copyFile: async (path: string, destination: string) => {
		try {
			const result = await invoke<FileOperationResult>("copy_file", {
				path,
				destination,
			});
			await get().refresh();
			return result;
		} catch (e) {
			throw new Error(String(e));
		}
	},

	createFolder: async (name: string) => {
		try {
			const { currentPath, refresh } = get();
			const result = await invoke<FileOperationResult>("create_folder", {
				path: currentPath,
				name,
			});
			await refresh();
			return result;
		} catch (e) {
			throw new Error(String(e));
		}
	},

	revealInFolder: async (path: string) => {
		try {
			const result = await invoke<FileOperationResult>("reveal_in_folder", {
				path,
			});
			return result;
		} catch (e) {
			throw new Error(String(e));
		}
	},

	// Bulk operations
	deleteSelected: async (permanent: boolean) => {
		const { selectedPaths, refresh } = get();
		let success = 0;
		let failed = 0;

		for (const path of selectedPaths) {
			try {
				await invoke<FileOperationResult>("delete_file", { path, permanent });
				success++;
			} catch {
				failed++;
			}
		}

		set({ selectedPaths: new Set(), selectedPath: null });
		await refresh();
		return { success, failed };
	},

	moveSelected: async (destination: string) => {
		const { selectedPaths, refresh } = get();
		let success = 0;
		let failed = 0;

		for (const path of selectedPaths) {
			try {
				await invoke<FileOperationResult>("move_file", { path, destination });
				success++;
			} catch {
				failed++;
			}
		}

		set({ selectedPaths: new Set(), selectedPath: null });
		await refresh();
		return { success, failed };
	},

	copySelected: async (destination: string) => {
		const { selectedPaths, refresh } = get();
		let success = 0;
		let failed = 0;

		for (const path of selectedPaths) {
			try {
				await invoke<FileOperationResult>("copy_file", { path, destination });
				success++;
			} catch {
				failed++;
			}
		}

		set({ selectedPaths: new Set() });
		await refresh();
		return { success, failed };
	},
}));
