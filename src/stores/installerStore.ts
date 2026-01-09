import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface InstallStrategy {
	method: string;
	requires_admin: boolean;
	available: boolean;
	tool_name: string;
	install_location: string;
	estimated_size_mb: number;
}

export interface InstallProgress {
	tool: string;
	method: string;
	current: number;
	total: number;
	percentage: number;
	stage: string;
	logs: string[];
}

export interface InstallResult {
	success: boolean;
	tool: string;
	method: string;
	message: string;
	installed_path?: string;
	version?: string;
}

export type InstallStatus = "idle" | "installing" | "success" | "failed";

interface ToolInstallState {
	status: InstallStatus;
	progress?: InstallProgress;
	result?: InstallResult;
	strategies?: InstallStrategy[];
	selectedMethod?: string;
}

interface InstallerStore {
	tools: Record<string, ToolInstallState>;

	// Actions
	getStrategies: (tool: string) => Promise<void>;
	install: (tool: string, method?: string) => Promise<void>;
	cancelInstallation: (tool: string) => Promise<void>;
	reset: (tool: string) => void;
	setSelectedMethod: (tool: string, method: string) => void;
}

export const useInstallerStore = create<InstallerStore>((set, get) => ({
	tools: {},

	getStrategies: async (tool: string) => {
		try {
			const strategies = await invoke<InstallStrategy[]>("get_install_strategies", { tool });

			set((state) => ({
				tools: {
					...state.tools,
					[tool]: {
						...state.tools[tool],
						strategies,
						selectedMethod: strategies[0]?.method, // Default to first available
					},
				},
			}));
		} catch (error) {
			console.error(`Failed to get strategies for ${tool}:`, error);
		}
	},

	install: async (tool: string, method?: string) => {
		const state = get();
		const toolState = state.tools[tool];
		const installMethod = method || toolState?.selectedMethod;

		// Set installing status
		set((state) => ({
			tools: {
				...state.tools,
				[tool]: {
					...state.tools[tool],
					status: "installing",
					progress: {
						tool,
						method: installMethod || "unknown",
						current: 0,
						total: 100,
						percentage: 0,
						stage: "Starting installation...",
						logs: [],
					},
					result: undefined,
				},
			},
		}));

		// Listen for progress events
		const progressUnlisten = await listen<InstallProgress>(
			`install-progress-${tool}`,
			(event) => {
				set((state) => ({
					tools: {
						...state.tools,
						[tool]: {
							...state.tools[tool],
							progress: event.payload,
						},
					},
				}));
			}
		);

		// Listen for completion event
		const completeUnlisten = await listen<InstallResult>(
			`install-complete-${tool}`,
			(event) => {
				set((state) => ({
					tools: {
						...state.tools,
						[tool]: {
							...state.tools[tool],
							status: event.payload.success ? "success" : "failed",
							result: event.payload,
						},
					},
				}));

				// Clean up listeners
				progressUnlisten();
				completeUnlisten();
			}
		);

		try {
			const result = await invoke<InstallResult>("install_dependency", {
				tool,
				method: installMethod,
			});

			// Update final state
			set((state) => ({
				tools: {
					...state.tools,
					[tool]: {
						...state.tools[tool],
						status: result.success ? "success" : "failed",
						result,
					},
				},
			}));

			// Clean up listeners if completion event didn't fire
			progressUnlisten();
			completeUnlisten();
		} catch (error) {
			console.error(`Installation failed for ${tool}:`, error);

			set((state) => ({
				tools: {
					...state.tools,
					[tool]: {
						...state.tools[tool],
						status: "failed",
						result: {
							success: false,
							tool,
							method: installMethod || "unknown",
							message: error instanceof Error ? error.message : String(error),
						},
					},
				},
			}));

			// Clean up listeners
			progressUnlisten();
			completeUnlisten();
		}
	},

	cancelInstallation: async (tool: string) => {
		try {
			await invoke("cancel_installation", { tool });

			set((state) => ({
				tools: {
					...state.tools,
					[tool]: {
						...state.tools[tool],
						status: "idle",
					},
				},
			}));
		} catch (error) {
			console.error(`Failed to cancel installation for ${tool}:`, error);
		}
	},

	reset: (tool: string) => {
		set((state) => ({
			tools: {
				...state.tools,
				[tool]: {
					status: "idle",
					strategies: state.tools[tool]?.strategies,
					selectedMethod: state.tools[tool]?.selectedMethod,
				},
			},
		}));
	},

	setSelectedMethod: (tool: string, method: string) => {
		set((state) => ({
			tools: {
				...state.tools,
				[tool]: {
					...state.tools[tool],
					selectedMethod: method,
				},
			},
		}));
	},
}));
