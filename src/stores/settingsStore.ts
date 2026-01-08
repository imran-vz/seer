/**
 * Settings store for managing user preferences
 * Persists settings to SQLite database via tauri-plugin-sql
 */

import { invoke } from "@tauri-apps/api/core";
import Database from "@tauri-apps/plugin-sql";
import { create } from "zustand";

export interface AppSettings {
	// General settings
	defaultDownloadLocation: string;
	startInLastDirectory: boolean;
	lastDirectory: string;

	// UI settings
	showHiddenFiles: boolean;
	confirmBeforeDelete: boolean;
	useTrashByDefault: boolean;
	rightPanelVisible: boolean;

	// Cache settings
	enableCaching: boolean;
	cacheExpirationDays: number;

	// Performance settings
	maxParallelJobs: number;
}

const defaultSettings: AppSettings = {
	// General settings
	defaultDownloadLocation: "",
	startInLastDirectory: true,
	lastDirectory: "",

	// UI settings
	showHiddenFiles: false,
	confirmBeforeDelete: true,
	useTrashByDefault: true,
	rightPanelVisible: true,

	// Cache settings
	enableCaching: true,
	cacheExpirationDays: 30,

	// Performance settings
	maxParallelJobs: 4,
};

// Map frontend setting keys to database keys
const keyMap: Record<keyof AppSettings, string> = {
	defaultDownloadLocation: "default_download_location",
	startInLastDirectory: "start_in_last_directory",
	lastDirectory: "last_directory",
	showHiddenFiles: "show_hidden_files",
	confirmBeforeDelete: "confirm_before_delete",
	useTrashByDefault: "use_trash_by_default",
	rightPanelVisible: "right_panel_visible",
	enableCaching: "enable_caching",
	cacheExpirationDays: "cache_expiration_days",
	maxParallelJobs: "max_parallel_jobs",
};

// Reverse map for database to frontend keys
const reverseKeyMap: Record<string, keyof AppSettings> = Object.fromEntries(
	Object.entries(keyMap).map(([k, v]) => [v, k as keyof AppSettings]),
);

interface PathValidationResult {
	valid: boolean;
	exists: boolean;
	created: boolean;
	path: string;
	error: string | null;
}

interface SettingsState {
	settings: AppSettings;
	loading: boolean;
	initialized: boolean;
	initializing: boolean;
	error: string | null;

	// Actions
	initialize: () => Promise<void>;
	updateSetting: <K extends keyof AppSettings>(
		key: K,
		value: AppSettings[K],
	) => Promise<void>;
	updateSettings: (updates: Partial<AppSettings>) => Promise<void>;
	resetSettings: () => Promise<void>;
	setLastDirectory: (path: string) => Promise<void>;

	// Path utilities
	validatePath: (
		path: string,
		createIfMissing?: boolean,
	) => Promise<PathValidationResult>;
	pickFolder: (title?: string, defaultPath?: string) => Promise<string | null>;
	getDefaultDownloadsDir: () => Promise<string>;
}

let db: Database | null = null;
let dbPromise: Promise<Database> | null = null;

async function getDatabase(): Promise<Database> {
	// Return existing connection
	if (db) return db;

	// Return in-progress initialization promise to prevent race conditions
	if (dbPromise) return dbPromise;

	// Start initialization and store the promise
	dbPromise = Database.load("sqlite:seer.db").then((database) => {
		db = database;
		return database;
	});

	return dbPromise;
}

// Convert value to database string format
function valueToDb(value: unknown): string {
	if (typeof value === "boolean") {
		return value ? "true" : "false";
	}
	return String(value);
}

// Convert database string to typed value
function dbToValue<K extends keyof AppSettings>(
	key: K,
	dbValue: string,
): AppSettings[K] {
	const defaultValue = defaultSettings[key];

	if (typeof defaultValue === "boolean") {
		return (dbValue === "true") as AppSettings[K];
	}
	if (typeof defaultValue === "number") {
		const num = Number.parseInt(dbValue, 10);
		return (Number.isNaN(num) ? defaultValue : num) as AppSettings[K];
	}
	return dbValue as AppSettings[K];
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
	settings: defaultSettings,
	loading: false,
	initialized: false,
	initializing: false,
	error: null,

	initialize: async () => {
		// Prevent double initialization - check both flags
		const state = get();
		if (state.initialized || state.initializing) return;

		// Set initializing flag immediately to prevent race conditions
		set({ initializing: true, loading: true, error: null });
		try {
			const database = await getDatabase();

			// Load all settings from database
			const rows = await database.select<{ key: string; value: string }[]>(
				"SELECT key, value FROM settings",
			);

			const loadedSettings = { ...defaultSettings };
			console.log(
				"[SettingsStore] Loading settings from DB, found rows:",
				rows,
			);
			for (const row of rows) {
				const frontendKey = reverseKeyMap[row.key];
				if (frontendKey) {
					const value = dbToValue(frontendKey, row.value);

					// Use type assertion since we know the keys match
					(loadedSettings as Record<string, unknown>)[frontendKey] = value;
					console.log(
						`[SettingsStore] Loaded ${frontendKey} = ${JSON.stringify(value)} (from DB: ${row.value})`,
					);
				}
			}

			set({
				settings: loadedSettings,
				loading: false,
				initialized: true,
				initializing: false,
			});

			console.log(
				"[SettingsStore] Settings loaded from database:",
				loadedSettings,
			);
		} catch (error) {
			console.error("[SettingsStore] Failed to initialize:", error);
			set({
				error: error instanceof Error ? error.message : String(error),
				loading: false,
				initialized: true, // Mark as initialized even on error to prevent infinite retries
				initializing: false,
			});
		}
	},

	updateSetting: async (key, value) => {
		const dbKey = keyMap[key];
		const dbValue = valueToDb(value);

		try {
			const database = await getDatabase();

			// Upsert the setting
			await database.execute(
				`INSERT INTO settings (key, value, updated_at)
				 VALUES ($1, $2, datetime('now'))
				 ON CONFLICT(key) DO UPDATE SET value = $2, updated_at = datetime('now')`,
				[dbKey, dbValue],
			);

			// Update local state
			set({
				settings: {
					...get().settings,
					[key]: value,
				},
			});

			// Special handling for maxParallelJobs - update backend
			if (key === "maxParallelJobs") {
				try {
					await invoke("set_max_parallel_jobs", { count: value });
					console.log(
						`[SettingsStore] Updated backend max_parallel_jobs = ${value}`,
					);
				} catch (error) {
					console.error(
						"[SettingsStore] Failed to update backend max_parallel_jobs:",
						error,
					);
				}
			}

			console.log(`[SettingsStore] Updated ${key} = ${value}`);
		} catch (error) {
			console.error(`[SettingsStore] Failed to update ${key}:`, error);
			throw error;
		}
	},

	updateSettings: async (updates) => {
		const database = await getDatabase();

		try {
			// Update each setting
			for (const [key, value] of Object.entries(updates)) {
				const typedKey = key as keyof AppSettings;
				const dbKey = keyMap[typedKey];
				const dbValue = valueToDb(value);

				await database.execute(
					`INSERT INTO settings (key, value, updated_at)
					 VALUES ($1, $2, datetime('now'))
					 ON CONFLICT(key) DO UPDATE SET value = $2, updated_at = datetime('now')`,
					[dbKey, dbValue],
				);
			}

			// Update local state
			set({
				settings: {
					...get().settings,
					...updates,
				},
			});

			console.log("[SettingsStore] Updated multiple settings");
		} catch (error) {
			console.error("[SettingsStore] Failed to update settings:", error);
			throw error;
		}
	},

	resetSettings: async () => {
		try {
			const database = await getDatabase();

			// Reset all settings to defaults
			for (const [key, dbKey] of Object.entries(keyMap)) {
				const typedKey = key as keyof AppSettings;
				const dbValue = valueToDb(defaultSettings[typedKey]);

				await database.execute(
					`INSERT INTO settings (key, value, updated_at)
					 VALUES ($1, $2, datetime('now'))
					 ON CONFLICT(key) DO UPDATE SET value = $2, updated_at = datetime('now')`,
					[dbKey, dbValue],
				);
			}

			set({ settings: defaultSettings });
			console.log("[SettingsStore] Settings reset to defaults");
		} catch (error) {
			console.error("[SettingsStore] Failed to reset settings:", error);
			throw error;
		}
	},

	setLastDirectory: async (path) => {
		console.log("[SettingsStore] setLastDirectory called with:", path);
		await get().updateSetting("lastDirectory", path);
		console.log(
			"[SettingsStore] lastDirectory updated, current value:",
			get().settings.lastDirectory,
		);
	},

	validatePath: async (path, createIfMissing = false) => {
		try {
			const result = await invoke<PathValidationResult>("validate_path", {
				path,
				createIfMissing,
			});
			return result;
		} catch (error) {
			return {
				valid: false,
				exists: false,
				created: false,
				path,
				error: error instanceof Error ? error.message : String(error),
			};
		}
	},

	pickFolder: async (title, defaultPath) => {
		try {
			const result = await invoke<string | null>("pick_folder", {
				title: title ?? "Select Folder",
				defaultPath: defaultPath ?? null,
			});
			return result;
		} catch (error) {
			console.error("[SettingsStore] Failed to pick folder:", error);
			return null;
		}
	},

	getDefaultDownloadsDir: async () => {
		try {
			return await invoke<string>("get_default_downloads_dir");
		} catch (error) {
			console.error("[SettingsStore] Failed to get downloads dir:", error);
			return "~/Downloads";
		}
	},
}));
