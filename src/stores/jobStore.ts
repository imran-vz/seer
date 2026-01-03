/**
 * Job store for managing background jobs
 * Integrates with SQLite database for persistence
 */

import { create } from "zustand";
import {
	cancelJob,
	cleanupOldJobs,
	completeJob,
	createJob,
	failJob,
	getJobStatistics,
	getJobsByFilePath,
	getJobsByStatus,
	getJobsByType,
	getPendingJobs,
	getRecentJobs,
	getRunningJobs,
	startJob,
	updateJob,
} from "@/lib/database";
import type {
	CreateJobParams,
	Job,
	JobStatistics,
	JobType,
	UpdateJobParams,
} from "@/types/database";

interface JobState {
	// State
	jobs: Job[];
	runningJobs: Job[];
	pendingJobs: Job[];
	statistics: JobStatistics | null;
	loading: boolean;
	error: string | null;

	// Actions
	refresh: () => Promise<void>;
	createJob: (params: CreateJobParams) => Promise<Job>;
	updateJob: (id: number, updates: UpdateJobParams) => Promise<Job | null>;
	startJob: (id: number) => Promise<Job | null>;
	completeJob: (
		id: number,
		result?: Record<string, unknown>,
	) => Promise<Job | null>;
	failJob: (id: number, errorMessage: string) => Promise<Job | null>;
	cancelJob: (id: number) => Promise<Job | null>;

	// Queries
	getJobsByFilePath: (filePath: string) => Promise<Job[]>;
	getJobsByType: (jobType: JobType) => Promise<Job[]>;
	getJobsByStatus: (
		status: "pending" | "running" | "completed" | "failed" | "cancelled",
	) => Promise<Job[]>;

	// Maintenance
	cleanupOldJobs: (daysOld?: number) => Promise<number>;
}

export const useJobStore = create<JobState>((set, get) => ({
	jobs: [],
	runningJobs: [],
	pendingJobs: [],
	statistics: null,
	loading: false,
	error: null,

	refresh: async () => {
		set({ loading: true, error: null });
		try {
			const [jobs, running, pending, stats] = await Promise.all([
				getRecentJobs(100),
				getRunningJobs(),
				getPendingJobs(),
				getJobStatistics(),
			]);

			set({
				jobs,
				runningJobs: running,
				pendingJobs: pending,
				statistics: stats,
				loading: false,
			});
		} catch (error) {
			console.error("[JobStore] Failed to refresh:", error);
			set({
				error: error instanceof Error ? error.message : String(error),
				loading: false,
			});
		}
	},

	createJob: async (params: CreateJobParams) => {
		try {
			const job = await createJob(params);
			console.log(`[JobStore] Created job ${job.id}: ${job.job_type}`);

			// Refresh to update lists
			await get().refresh();

			return job;
		} catch (error) {
			console.error("[JobStore] Failed to create job:", error);
			throw error;
		}
	},

	updateJob: async (id: number, updates: UpdateJobParams) => {
		try {
			const job = await updateJob(id, updates);
			console.log(`[JobStore] Updated job ${id}`);

			// Refresh to update lists
			await get().refresh();

			return job;
		} catch (error) {
			console.error("[JobStore] Failed to update job:", error);
			throw error;
		}
	},

	startJob: async (id: number) => {
		try {
			const job = await startJob(id);
			console.log(`[JobStore] Started job ${id}`);

			// Refresh to update lists
			await get().refresh();

			return job;
		} catch (error) {
			console.error("[JobStore] Failed to start job:", error);
			throw error;
		}
	},

	completeJob: async (id: number, result?: Record<string, unknown>) => {
		try {
			const job = await completeJob(id, result);
			console.log(`[JobStore] Completed job ${id}`);

			// Refresh to update lists
			await get().refresh();

			return job;
		} catch (error) {
			console.error("[JobStore] Failed to complete job:", error);
			throw error;
		}
	},

	failJob: async (id: number, errorMessage: string) => {
		try {
			const job = await failJob(id, errorMessage);
			console.log(`[JobStore] Failed job ${id}: ${errorMessage}`);

			// Refresh to update lists
			await get().refresh();

			return job;
		} catch (error) {
			console.error("[JobStore] Failed to fail job:", error);
			throw error;
		}
	},

	cancelJob: async (id: number) => {
		try {
			const job = await cancelJob(id);
			console.log(`[JobStore] Cancelled job ${id}`);

			// Refresh to update lists
			await get().refresh();

			return job;
		} catch (error) {
			console.error("[JobStore] Failed to cancel job:", error);
			throw error;
		}
	},

	getJobsByFilePath: async (filePath: string) => {
		try {
			return await getJobsByFilePath(filePath);
		} catch (error) {
			console.error("[JobStore] Failed to get jobs by file path:", error);
			throw error;
		}
	},

	getJobsByType: async (jobType: JobType) => {
		try {
			return await getJobsByType(jobType);
		} catch (error) {
			console.error("[JobStore] Failed to get jobs by type:", error);
			throw error;
		}
	},

	getJobsByStatus: async (
		status: "pending" | "running" | "completed" | "failed" | "cancelled",
	) => {
		try {
			return await getJobsByStatus(status);
		} catch (error) {
			console.error("[JobStore] Failed to get jobs by status:", error);
			throw error;
		}
	},

	cleanupOldJobs: async (daysOld = 30) => {
		try {
			const count = await cleanupOldJobs(daysOld);
			console.log(`[JobStore] Cleaned up ${count} old jobs`);

			// Refresh to update lists
			await get().refresh();

			return count;
		} catch (error) {
			console.error("[JobStore] Failed to cleanup old jobs:", error);
			throw error;
		}
	},
}));
