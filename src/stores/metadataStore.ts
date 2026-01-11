import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import { invalidateFileMetadata } from "@/lib/fileMetadataCache";
import type {
	MetadataOperation,
	MetadataSnapshot,
	MetadataToolAvailability,
	MetadataUpdateResult,
} from "@/types/metadata";

interface MetadataState {
	currentPath: string | null;
	snapshot: MetadataSnapshot | null;
	loading: boolean;
	saving: boolean;
	error: string | null;
	pendingOperations: MetadataOperation[];
	toolAvailability: MetadataToolAvailability | null;
	load: (path: string) => Promise<void>;
	reset: () => void;
	stageOperation: (operation: MetadataOperation) => void;
	removeOperation: (index: number) => void;
	clearOperations: () => void;
	applyOperations: () => Promise<MetadataUpdateResult | null>;
}

const initialState: Omit<
	MetadataState,
	| "load"
	| "reset"
	| "stageOperation"
	| "removeOperation"
	| "clearOperations"
	| "applyOperations"
> = {
	currentPath: null,
	snapshot: null,
	loading: false,
	saving: false,
	error: null,
	pendingOperations: [],
	toolAvailability: null,
};

export const useMetadataStore = create<MetadataState>((set, get) => ({
	...initialState,

	load: async (path: string) => {
		set({ loading: true, error: null, currentPath: path });
		try {
			const snapshot = await invoke<MetadataSnapshot>("list_metadata", {
				path,
			});
			set({
				snapshot,
				toolAvailability: snapshot.toolAvailability,
				loading: false,
				pendingOperations: [],
			});
		} catch (e) {
			set({ error: String(e), loading: false, snapshot: null });
		}
	},

	reset: () => set(initialState),

	stageOperation: (operation) =>
		set((state) => ({
			pendingOperations: [...state.pendingOperations, operation],
		})),

	removeOperation: (index) =>
		set((state) => ({
			pendingOperations: state.pendingOperations.filter((_, i) => i !== index),
		})),

	clearOperations: () => set({ pendingOperations: [] }),

	applyOperations: async () => {
		const { currentPath, pendingOperations } = get();
		if (!currentPath || pendingOperations.length === 0) {
			return null;
		}

		set({ saving: true, error: null });

		try {
			const result = await invoke<MetadataUpdateResult>("update_metadata", {
				path: currentPath,
				operations: pendingOperations,
			});

			await invalidateFileMetadata(currentPath);
			await get().load(currentPath);
			set({ saving: false, pendingOperations: [] });
			return result;
		} catch (e) {
			set({ saving: false, error: String(e) });
			return null;
		}
	},
}));
