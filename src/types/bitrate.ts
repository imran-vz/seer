export interface BitrateDataPoint {
	timestamp: number;
	bitrate: number;
	frame_type?: string;
}

export interface JobStatus {
	job_id: number;
	path: string;
	running_seconds: number;
}

export interface JobInfo {
	job_id: number;
	path: string;
	state: string; // Format: "queued:job_type" or "running:job_type" (e.g., "queued:bitrate_analysis", "running:stream_removal")
	queued_seconds?: number;
	running_seconds?: number;
	progress_current?: number;
	progress_total?: number;
	progress_percentage?: number;
	progress_stage?: string;
}

export interface QueueStatus {
	queued: JobInfo[];
	running: JobInfo[];
	max_parallel: number;
}

export interface PeakInterval {
	start_time: number;
	end_time: number;
	peak_bitrate: number;
	duration: number;
}

export interface BitrateStatistics {
	min_bitrate: number;
	max_bitrate: number;
	avg_bitrate: number;
	median_bitrate: number;
	std_deviation: number;
	peak_intervals: PeakInterval[];
	total_frames: number;
}

export interface BitrateAnalysis {
	path: string;
	stream_index: number;
	stream_type:
		| "video"
		| "audio"
		| "subtitle"
		| "attachment"
		| "data"
		| "unknown";
	duration: number;
	data_points: BitrateDataPoint[];
	statistics: BitrateStatistics;
}

export interface StreamContribution {
	stream_index: number;
	stream_type: string;
	codec_name: string;
	percentage: number;
	data_points: BitrateDataPoint[];
}

export interface OverallBitrateAnalysis {
	path: string;
	duration: number;
	data_points: BitrateDataPoint[];
	statistics: BitrateStatistics;
	stream_contributions: StreamContribution[];
	from_cache: boolean;
}
