import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";

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

	// Actions
	setCurrentPath: (path: string) => void;
	setSelectedPath: (path: string | null) => void;
	loadDirectory: (path: string) => Promise<void>;
	refresh: () => Promise<void>;
	goUp: () => Promise<void>;
	navigate: (entry: FileEntry) => Promise<void>;
	selectFile: (entry: FileEntry) => void;
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

	setCurrentPath: (path) => set({ currentPath: path }),

	setSelectedPath: (path) => set({ selectedPath: path }),

	loadDirectory: async (path: string) => {
		set({ loading: true, error: null, selectedPaths: new Set() });
		try {
			const entries = await invoke<FileEntry[]>("list_directory", { path });
			set({ files: entries, currentPath: path, loading: false });
		} catch (e) {
			set({ error: String(e), loading: false });
		}
	},

	refresh: async () => {
		const { currentPath, loadDirectory } = get();
		await loadDirectory(currentPath);
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

	initialize: async () => {
		const { loadDirectory } = get();
		try {
			const home = await invoke<string>("get_home_dir");
			await loadDirectory(home);
		} catch (e) {
			set({ error: String(e) });
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
