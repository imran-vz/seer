import type { MetadataEntry, MetadataOperation } from "@/types/metadata";

export interface DisplayTag extends Omit<MetadataEntry, "origin"> {
	origin: MetadataEntry["origin"] | "pending";
	pending?: {
		op: MetadataOperation;
		index: number;
	};
}

export const COMMON_FIELDS = [
	"title",
	"artist",
	"album",
	"genre",
	"date",
	"description",
	"language",
	"copyright",
	"comment",
];

export const CRITICAL_FIELDS = [
	"duration",
	"bitrate",
	"rotate",
	"creation_time",
	"handler_name",
	"encoder",
	"major_brand",
	"minor_version",
	"compatible_brands",
];

export const isCritical = (key: string) =>
	CRITICAL_FIELDS.some((k) => key.toLowerCase().includes(k));

export const sortTags = (tags: DisplayTag[]) => {
	return [...tags].sort((a, b) => {
		const aKey = a.key.toLowerCase();
		const bKey = b.key.toLowerCase();

		const aCommon = COMMON_FIELDS.indexOf(aKey);
		const bCommon = COMMON_FIELDS.indexOf(bKey);

		if (aCommon !== -1 && bCommon !== -1) return aCommon - bCommon;
		if (aCommon !== -1) return -1;
		if (bCommon !== -1) return 1;

		return aKey.localeCompare(bKey);
	});
};
