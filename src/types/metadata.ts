export type MetadataAction = "set" | "delete";

export type MetadataScope = "format" | "stream" | "file";

export type MetadataOrigin = "ffprobe" | "exiftool";

export interface MetadataEntry {
	key: string;
	value: string;
	scope: MetadataScope;
	streamIndex?: number;
	origin: MetadataOrigin;
	editable: boolean;
}

export interface MetadataOperation {
	action: MetadataAction;
	key: string;
	value?: string;
	scope: MetadataScope;
	streamIndex?: number;
}

export interface MetadataToolAvailability {
	ffmpeg: boolean;
	ffprobe: boolean;
	exiftool: boolean;
}

export interface StreamSummary {
	index: number;
	codecType?: string;
	codecName?: string;
	codecLongName?: string;
	duration?: number;
	bitRate?: string;
}

export interface MetadataSnapshot {
	path: string;
	fileName: string;
	size: number;
	modified: string | null;
	created: string | null;
	extension: string | null;
	formatTags: MetadataEntry[];
	streamTags: MetadataEntry[];
	fileTags: MetadataEntry[];
	streamSummaries: StreamSummary[];
	toolAvailability: MetadataToolAvailability;
}

export interface MetadataUpdateResult {
	success: boolean;
	applied: MetadataOperation[];
	errors: string[];
}
