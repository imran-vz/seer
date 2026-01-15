import { Badge } from "@/components/ui/badge";
import type { MetadataScope } from "@/types/metadata";

import { TagRow } from "./TagRow";
import type { DisplayTag } from "./utils";

interface TagGroupProps {
	title: string;
	tags: DisplayTag[];
	scope: MetadataScope;
	streamIndex?: number;
	disabled?: boolean;
	onEdit: (
		key: string,
		value: string,
		scope: MetadataScope,
		streamIndex?: number,
	) => void;
	onDelete: (key: string, scope: MetadataScope, streamIndex?: number) => void;
	onUndo: (index: number) => void;
}

export function TagGroup({
	title,
	tags,
	scope,
	streamIndex,
	disabled,
	onEdit,
	onDelete,
	onUndo,
}: TagGroupProps) {
	if (tags.length === 0) return null;

	return (
		<div className="space-y-1">
			<div className="sticky top-0 z-10 flex items-center gap-2 bg-background py-2">
				<Badge variant="outline" className="font-normal text-muted-foreground">
					{tags.length}
				</Badge>
				<h4 className="font-medium text-sm">{title}</h4>
			</div>
			<div className="rounded-md border border-border bg-card">
				{tags.map((tag, i) => (
					<TagRow
						key={`${tag.key}-${i}`}
						tag={tag}
						isLast={i === tags.length - 1}
						disabled={disabled}
						onEdit={(val) => onEdit(tag.key, val, scope, streamIndex)}
						onDelete={() => onDelete(tag.key, scope, streamIndex)}
						onUndo={onUndo}
					/>
				))}
			</div>
		</div>
	);
}
