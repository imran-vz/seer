import { Plus } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import type { MetadataScope } from "@/types/metadata";

interface AddTagDialogProps {
	disabled?: boolean;
	onAdd: (
		key: string,
		value: string,
		scope: MetadataScope,
		streamIndex?: number,
	) => void;
}

export function AddTagDialog({ disabled, onAdd }: AddTagDialogProps) {
	const [open, setOpen] = useState(false);
	const [key, setKey] = useState("");
	const [value, setValue] = useState("");
	const [scope, setScope] = useState<MetadataScope>("format");
	const [streamIndex, setStreamIndex] = useState("");

	const handleAdd = () => {
		if (!key) return;
		const streamIdx =
			scope === "stream" && streamIndex
				? Number.parseInt(streamIndex, 10)
				: undefined;
		onAdd(key, value, scope, streamIdx);
		setOpen(false);
		// Reset form
		setKey("");
		setValue("");
		setScope("format");
		setStreamIndex("");
	};

	return (
		<Dialog open={open} onOpenChange={setOpen}>
			<DialogTrigger asChild>
				<Button
					size="sm"
					variant="outline"
					className="h-8 gap-2"
					disabled={disabled}
				>
					<Plus className="h-3.5 w-3.5" />
					Add Tag
				</Button>
			</DialogTrigger>
			<DialogContent>
				<DialogHeader>
					<DialogTitle>Add Metadata Tag</DialogTitle>
					<DialogDescription>
						Add a new metadata field to the file.
					</DialogDescription>
				</DialogHeader>
				<div className="grid gap-4 py-4">
					<div className="grid grid-cols-4 items-center gap-4">
						<label htmlFor="tag-key" className="text-right text-sm">
							Key
						</label>
						<Input
							id="tag-key"
							value={key}
							onChange={(e) => setKey(e.target.value)}
							className="col-span-3"
							placeholder="e.g. title"
						/>
					</div>
					<div className="grid grid-cols-4 items-center gap-4">
						<label htmlFor="tag-value" className="text-right text-sm">
							Value
						</label>
						<Input
							id="tag-value"
							value={value}
							onChange={(e) => setValue(e.target.value)}
							className="col-span-3"
							placeholder="Value"
						/>
					</div>
					<div className="grid grid-cols-4 items-center gap-4">
						<label htmlFor="tag-scope" className="text-right text-sm">
							Scope
						</label>
						<Select
							value={scope}
							onValueChange={(v) => setScope(v as MetadataScope)}
						>
							<SelectTrigger className="col-span-3">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="format">Format (Global)</SelectItem>
								<SelectItem value="stream">Stream</SelectItem>
								<SelectItem value="file">File (Exif/Info)</SelectItem>
							</SelectContent>
						</Select>
					</div>
					{scope === "stream" && (
						<div className="grid grid-cols-4 items-center gap-4">
							<label htmlFor="tag-stream" className="text-right text-sm">
								Stream
							</label>
							<Input
								id="tag-stream"
								type="number"
								value={streamIndex}
								onChange={(e) => setStreamIndex(e.target.value)}
								className="col-span-3"
								placeholder="Stream Index (e.g. 0)"
							/>
						</div>
					)}
				</div>
				<DialogFooter>
					<Button variant="outline" onClick={() => setOpen(false)}>
						Cancel
					</Button>
					<Button onClick={handleAdd} disabled={!key}>
						Add Tag
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
