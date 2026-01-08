/**
 * Format bytes to human-readable size string
 */
export function formatSize(bytes: number): string {
	if (bytes === 0) return "-";
	const units = ["B", "KB", "MB", "GB"];
	const i = Math.floor(Math.log(bytes) / Math.log(1024));
	return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
}

const IMAGE_EXTENSIONS = [
	"jpg",
	"jpeg",
	"png",
	"gif",
	"bmp",
	"webp",
	"tiff",
	"heic",
	"heif",
	"svg",
];

const VIDEO_AUDIO_EXTENSIONS = [
	"mp4",
	"mkv",
	"avi",
	"mov",
	"wmv",
	"flv",
	"webm",
	"m4v",
	"mp3",
	"flac",
	"wav",
	"aac",
	"ogg",
	"wma",
	"m4a",
	"opus",
];

export function isImageFile(filePath: string | null): boolean {
	if (!filePath) return false;

	const extension = filePath.split(".").pop()?.toLowerCase();
	return extension ? IMAGE_EXTENSIONS.includes(extension) : false;
}

export function isVideoOrAudioFile(filePath: string | null): boolean {
	if (!filePath) return false;

	const extension = filePath.split(".").pop()?.toLowerCase();
	return extension ? VIDEO_AUDIO_EXTENSIONS.includes(extension) : false;
}
