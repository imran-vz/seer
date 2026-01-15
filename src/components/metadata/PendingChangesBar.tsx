import { Save, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";

interface PendingChangesBarProps {
	pendingCount: number;
	saving: boolean;
	onApply: () => void;
	onClear: () => void;
}

export function PendingChangesBar({
	pendingCount,
	saving,
	onApply,
	onClear,
}: PendingChangesBarProps) {
	if (pendingCount === 0) return null;

	return (
		<div className="slide-in-from-bottom-2 absolute right-4 bottom-4 left-4 z-10 animate-in">
			<div className="flex items-center justify-between rounded-lg border border-primary/20 bg-primary/10 p-4 shadow-lg backdrop-blur-md">
				<div className="flex items-center gap-3">
					<div className="flex h-8 w-8 items-center justify-center rounded-full bg-primary text-primary-foreground">
						<Save className="h-4 w-4" />
					</div>
					<div>
						<p className="font-medium text-sm">Unsaved Changes</p>
						<p className="text-muted-foreground text-xs">
							{pendingCount} pending operation
							{pendingCount !== 1 ? "s" : ""}
						</p>
					</div>
				</div>
				<div className="flex items-center gap-2">
					<TooltipProvider>
						<Tooltip>
							<TooltipTrigger asChild>
								<Button
									variant="ghost"
									size="sm"
									className="h-8 w-8 p-0 text-muted-foreground hover:text-foreground"
									onClick={onClear}
								>
									<X className="h-4 w-4" />
								</Button>
							</TooltipTrigger>
							<TooltipContent>Discard All</TooltipContent>
						</Tooltip>
					</TooltipProvider>
					<Button size="sm" onClick={onApply} disabled={saving}>
						{saving ? "Applying..." : "Apply Changes"}
					</Button>
				</div>
			</div>
		</div>
	);
}
