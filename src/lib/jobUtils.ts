/**
 * Utility functions for working with job states and types
 */

export type JobState = "queued" | "running";
export type JobTypeName = "bitrate_analysis" | "stream_removal";

export interface ParsedJobState {
	state: JobState;
	jobType: JobTypeName;
}

/**
 * Parse job state string from backend
 * Format: "queued:job_type" or "running:job_type"
 * Example: "queued:bitrate_analysis" or "running:stream_removal"
 */
export function parseJobState(state: string): ParsedJobState | null {
	const parts = state.split(":");
	if (parts.length !== 2) return null;

	const [stateStr, jobType] = parts;
	if (
		(stateStr !== "queued" && stateStr !== "running") ||
		(jobType !== "bitrate_analysis" && jobType !== "stream_removal")
	) {
		return null;
	}

	return {
		state: stateStr as JobState,
		jobType: jobType as JobTypeName,
	};
}

/**
 * Get friendly display name for job type
 */
export function getJobTypeName(jobType: JobTypeName): string {
	switch (jobType) {
		case "bitrate_analysis":
			return "Bitrate Analysis";
		case "stream_removal":
			return "Stream Removal";
		default:
			return jobType;
	}
}

/**
 * Get emoji icon for job type
 */
export function getJobTypeIcon(jobType: JobTypeName): string {
	switch (jobType) {
		case "bitrate_analysis":
			return "📊";
		case "stream_removal":
			return "✂️";
		default:
			return "⚙️";
	}
}

/**
 * Check if job state matches criteria
 */
export function isJobState(
	state: string,
	targetState?: JobState,
	targetJobType?: JobTypeName,
): boolean {
	const parsed = parseJobState(state);
	if (!parsed) return false;

	if (targetState && parsed.state !== targetState) return false;
	if (targetJobType && parsed.jobType !== targetJobType) return false;

	return true;
}
