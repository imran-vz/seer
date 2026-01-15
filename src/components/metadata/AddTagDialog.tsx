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
	filename?: string;
	onAdd: (
		key: string,
		value: string,
		scope: MetadataScope,
		streamIndex?: number,
	) => void;
}

const COMMON_TAGS = [
	"title",
	"language",
	"handler_name",
	"genre",
	"date",
	"creation_time",
	"artist",
	"album",
	"comment",
	"copyright",
	"encoder",
	"publisher",
	"encoded_by",
];

const inferTitle = (filename: string) => {
	// Remove extension
	const base = filename.substring(0, filename.lastIndexOf(".")) || filename;

	// Check for Year pattern: (YYYY) or [YYYY]
	const yearPattern = /([[(]?\d{4}[\])]?)/;
	const yearMatch = base.match(yearPattern);

	if (yearMatch?.index && yearMatch.index > 0) {
		const year = Number.parseInt(yearMatch[0].replace(/[[(\])]/g, ""));
		if (year > 1900 && year < 2100) {
			return base.substring(0, yearMatch.index + yearMatch[0].length).trim();
		}
	}

	// Check for Season/Episode pattern: SxxExx
	const seasonPattern = /S\d{2}E\d{2}/i;
	const seasonMatch = base.match(seasonPattern);
	if (seasonMatch?.index) {
		const title = base.substring(0, seasonMatch.index);
		return title.replace(/\./g, " ").trim();
	}

	return base;
};

export function AddTagDialog({ disabled, filename, onAdd }: AddTagDialogProps) {
	const [open, setOpen] = useState(false);
	const [key, setKey] = useState("");
	const [value, setValue] = useState("");
	const [scope, setScope] = useState<MetadataScope>("format");
	const [streamIndex, setStreamIndex] = useState("");
	const [showSuggestions, setShowSuggestions] = useState(false);

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
		setShowSuggestions(false);
	};

	const handleSelectKey = (selectedKey: string) => {
		setKey(selectedKey);
		setShowSuggestions(false);
		if (selectedKey.toLowerCase() === "title" && filename && !value.trim()) {
			setValue(inferTitle(filename));
		}
	};

	const filteredTags = COMMON_TAGS.filter((tag) =>
		tag.toLowerCase().includes(key.toLowerCase()),
	);

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
						<div className="relative col-span-3">
							<Input
								id="tag-key"
								value={key}
								onChange={(e) => {
									const newKey = e.target.value;
									setKey(newKey);
									setShowSuggestions(true);
									if (
										newKey.toLowerCase() === "title" &&
										filename &&
										!value.trim()
									) {
										setValue(inferTitle(filename));
									}
								}}
								onFocus={() => setShowSuggestions(true)}
								onBlur={() => setTimeout(() => setShowSuggestions(false), 200)}
								autoComplete="off"
								placeholder="e.g. title"
							/>
							{showSuggestions && filteredTags.length > 0 && (
								<div className="absolute top-full left-0 z-50 mt-1 max-h-48 w-full overflow-y-auto rounded-lg border border-input bg-popover p-1 text-popover-foreground shadow-md ring-1 ring-foreground/10">
									{filteredTags.map((tag) => (
										<div
											key={tag}
											className="relative flex cursor-pointer select-none items-center rounded-md px-2 py-1.5 text-xs/relaxed outline-none hover:bg-accent hover:text-accent-foreground"
											onClick={() => handleSelectKey(tag)}
										>
											{tag}
										</div>
									))}
								</div>
							)}
						</div>
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
