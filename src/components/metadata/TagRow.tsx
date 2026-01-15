import { AlertTriangle, Check, Pencil, Trash2, Undo2, X } from "lucide-react";
import { useEffect, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";

import { type DisplayTag, isCritical } from "./utils";

interface TagRowProps {
	tag: DisplayTag;
	isLast: boolean;
	disabled?: boolean;
	onEdit: (value: string) => void;
	onDelete: () => void;
	onUndo: (index: number) => void;
}

export function TagRow({
	tag,
	isLast,
	disabled,
	onEdit,
	onDelete,
	onUndo,
}: TagRowProps) {
	const [isEditing, setIsEditing] = useState(false);
	const [editValue, setEditValue] = useState(tag.value);
	const critical = isCritical(tag.key);

	// Reset edit value if tag value changes externally (e.g. from pending op)
	useEffect(() => {
		setEditValue(tag.pending?.op?.value ?? tag.value);
	}, [tag.value, tag.pending]);

	const pendingAction = tag.pending?.op?.action;
	const isDeleted = pendingAction === "delete";
	const isModified = pendingAction === "set";

	const handleSave = () => {
		onEdit(editValue);
		setIsEditing(false);
	};

	return (
		<div
			className={`group flex items-start gap-3 p-3 transition-colors hover:bg-muted/50 ${
				!isLast ? "border-border/50 border-b" : ""
			} ${isDeleted ? "bg-destructive/5 opacity-60" : ""} ${
				isModified ? "bg-primary/5" : ""
			}`}
		>
			<div className="mt-1 flex-1 space-y-1">
				<div className="flex items-center gap-2">
					<span
						className={`font-mono text-xs ${isDeleted ? "line-through" : ""}`}
					>
						{tag.key}
					</span>
					{critical && (
						<TooltipProvider>
							<Tooltip delayDuration={300}>
								<TooltipTrigger>
									<AlertTriangle className="h-3 w-3 text-amber-500" />
								</TooltipTrigger>
								<TooltipContent>
									<p>Critical field. Editing may affect playback.</p>
								</TooltipContent>
							</Tooltip>
						</TooltipProvider>
					)}
					{isModified && (
						<Badge variant="secondary" className="h-4 px-1 text-[10px]">
							Modified
						</Badge>
					)}
					{isDeleted && (
						<Badge variant="destructive" className="h-4 px-1 text-[10px]">
							Deleted
						</Badge>
					)}
				</div>
				{isEditing ? (
					<div className="flex items-center gap-2">
						<Input
							value={editValue}
							onChange={(e) => setEditValue(e.target.value)}
							className="h-7 text-xs"
							autoFocus
							onKeyDown={(e) => {
								if (e.key === "Enter") handleSave();
								if (e.key === "Escape") setIsEditing(false);
							}}
						/>
						<Button
							size="icon"
							variant="ghost"
							className="h-7 w-7"
							onClick={handleSave}
						>
							<Check className="h-3 w-3" />
						</Button>
						<Button
							size="icon"
							variant="ghost"
							className="h-7 w-7"
							onClick={() => setIsEditing(false)}
						>
							<X className="h-3 w-3" />
						</Button>
					</div>
				) : (
					<div
						className={`break-all text-muted-foreground text-sm ${
							isDeleted ? "line-through" : ""
						} ${isModified ? "font-medium text-foreground" : ""}`}
					>
						{tag.pending?.op?.value ?? tag.value}
					</div>
				)}
			</div>

			<div className="flex items-center opacity-0 transition-opacity focus-within:opacity-100 group-hover:opacity-100">
				{tag.pending ? (
					<TooltipProvider>
						<Tooltip>
							<TooltipTrigger asChild>
								<Button
									variant="ghost"
									size="icon"
									className="h-7 w-7"
									onClick={() => tag.pending && onUndo(tag.pending.index)}
								>
									<Undo2 className="h-3.5 w-3.5" />
								</Button>
							</TooltipTrigger>
							<TooltipContent>Undo change</TooltipContent>
						</Tooltip>
					</TooltipProvider>
				) : (
					!disabled && (
						<>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<Button
											variant="ghost"
											size="icon"
											className="h-7 w-7"
											onClick={() => setIsEditing(true)}
										>
											<Pencil className="h-3.5 w-3.5" />
										</Button>
									</TooltipTrigger>
									<TooltipContent>Edit</TooltipContent>
								</Tooltip>
							</TooltipProvider>
							<TooltipProvider>
								<Tooltip>
									<TooltipTrigger asChild>
										<Button
											variant="ghost"
											size="icon"
											className="h-7 w-7 hover:text-destructive"
											onClick={onDelete}
										>
											<Trash2 className="h-3.5 w-3.5" />
										</Button>
									</TooltipTrigger>
									<TooltipContent>Delete</TooltipContent>
								</Tooltip>
							</TooltipProvider>
						</>
					)
				)}
			</div>
		</div>
	);
}
