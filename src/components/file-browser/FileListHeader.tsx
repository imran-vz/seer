import { Checkbox } from "@/components/ui/checkbox";

interface FileListHeaderProps {
	allSelected: boolean;
	onSelectAllToggle: () => void;
}

export function FileListHeader({
	allSelected,
	onSelectAllToggle,
}: FileListHeaderProps) {
	return (
		<div className="grid grid-cols-[24px_20px_1fr_70px_130px] items-center gap-2 border-border/50 border-b px-3 py-1 font-medium text-[11px] text-muted-foreground uppercase tracking-wider">
			<div className="flex items-center justify-center">
				<Checkbox
					checked={allSelected}
					onCheckedChange={onSelectAllToggle}
					aria-label="Select all"
					className="h-3.5 w-3.5"
				/>
			</div>
			<span />
			<span>Name</span>
			<span className="text-right">Size</span>
			<span className="text-right">Modified</span>
		</div>
	);
}
