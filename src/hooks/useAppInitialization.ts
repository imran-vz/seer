/**
 * Centralized app initialization hook
 * Properly sequences the initialization of all stores to avoid race conditions
 */

import { useEffect, useState } from "react";
import { useInitDatabase } from "@/lib/useDatabase";
import { useFileBrowserStore } from "@/stores/fileBrowserStore";
import { useSettingsStore } from "@/stores/settingsStore";

export type InitializationStage =
	| "database"
	| "settings"
	| "file-browser"
	| "complete";

interface InitializationState {
	stage: InitializationStage;
	initialized: boolean;
	error: string | null;
}

export function useAppInitialization(): InitializationState {
	const [stage, setStage] = useState<InitializationStage>("database");
	const [error, setError] = useState<string | null>(null);

	// Database initialization
	const { initialized: dbInitialized, error: dbError } = useInitDatabase();

	// Settings store state
	const settingsInitialized = useSettingsStore((s) => s.initialized);
	const settingsInitializing = useSettingsStore((s) => s.initializing);
	const settingsError = useSettingsStore((s) => s.error);
	const initializeSettings = useSettingsStore((s) => s.initialize);

	// File browser store state
	const fileBrowserInitialized = useFileBrowserStore((s) => s.initialized);
	const fileBrowserInitializing = useFileBrowserStore((s) => s.initializing);
	const fileBrowserError = useFileBrowserStore((s) => s.error);
	const initializeFileBrowser = useFileBrowserStore((s) => s.initialize);

	// Stage 1: Wait for database
	useEffect(() => {
		if (dbError) {
			console.error("[AppInit] Database initialization failed:", dbError);
			setError(`Database initialization failed: ${dbError}`);
			return;
		}

		if (dbInitialized && stage === "database") {
			console.log("[AppInit] Database initialized, moving to settings stage");
			setStage("settings");
		}
	}, [dbInitialized, dbError, stage]);

	// Stage 2: Initialize settings
	useEffect(() => {
		if (stage !== "settings") return;

		if (settingsError) {
			console.error("[AppInit] Settings initialization failed:", settingsError);
			setError(`Settings initialization failed: ${settingsError}`);
			return;
		}

		if (!settingsInitialized && !settingsInitializing) {
			console.log("[AppInit] Initializing settings store");
			initializeSettings();
		} else if (settingsInitialized) {
			console.log(
				"[AppInit] Settings initialized, moving to file-browser stage",
			);
			setStage("file-browser");
		}
	}, [
		stage,
		settingsInitialized,
		settingsInitializing,
		settingsError,
		initializeSettings,
	]);

	// Stage 3: Initialize file browser
	useEffect(() => {
		if (stage !== "file-browser") return;

		if (fileBrowserError) {
			// File browser errors are non-fatal, just log them
			console.warn("[AppInit] File browser had an error:", fileBrowserError);
		}

		if (!fileBrowserInitialized && !fileBrowserInitializing) {
			console.log("[AppInit] Initializing file browser store");
			initializeFileBrowser();
		} else if (fileBrowserInitialized) {
			console.log(
				"[AppInit] File browser initialized, initialization complete",
			);
			setStage("complete");
		}
	}, [
		stage,
		fileBrowserInitialized,
		fileBrowserInitializing,
		fileBrowserError,
		initializeFileBrowser,
	]);

	const initialized = stage === "complete";

	return {
		stage,
		initialized,
		error,
	};
}
