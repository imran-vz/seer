import { invoke } from "@tauri-apps/api/core";
import { AlertCircle, Check, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";

interface RenamePreview {
	original_path: string;
	original_name: string;
	new_name: string;
	new_path: string;
	conflict: boolean;
	error: string | null;
}

interface BulkRenameResult {
	success: number;
	failed: number;
	errors: string[];
}

type RenamePatternType =
	| {
			type: "find_replace";
			find: string;
			replace: string;
			case_sensitive: boolean;
	  }
	| {
			type: "sequential";
			pattern: string;
			start: number;
			padding: number;
	  }
	| {
			type: "case_transform";
			mode: "lowercase" | "uppercase" | "titlecase";
	  }
	| {
			type: "template";
			template: string;
	  };

interface BulkRenameDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	paths: string[];
	onSuccess: () => void;
}

export function BulkRenameDialog({
	open,
	onOpenChange,
	paths,
	onSuccess,
}: BulkRenameDialogProps) {
	const [tab, setTab] = useState<
		"find_replace" | "sequential" | "case" | "template"
	>("find_replace");

	// Find & Replace state
	const [find, setFind] = useState("");
	const [replace, setReplace] = useState("");
	const [caseSensitive, setCaseSensitive] = useState(false);

	// Sequential state
	const [sequentialPattern, setSequentialPattern] = useState("{name}_{n}");
	const [start, setStart] = useState(1);
	const [padding, setPadding] = useState(3);

	// Case Transform state
	const [caseMode, setCaseMode] = useState<
		"lowercase" | "uppercase" | "titlecase"
	>("lowercase");

	// Template state
	const [template, setTemplate] = useState("{name}_{date}");

	// Common state
	const [autoRename, setAutoRename] = useState(true);
	const [previews, setPreviews] = useState<RenamePreview[]>([]);
	const [loading, setLoading] = useState(false);
	const [executing, setExecuting] = useState(false);
	const hasLoadedOnce = useRef(false);

	// Generate pattern based on current tab (wrapped in useCallback)
	const getPattern = useCallback((): RenamePatternType => {
		switch (tab) {
			case "find_replace":
				return {
					type: "find_replace",
					find,
					replace,
					case_sensitive: caseSensitive,
				};
			case "sequential":
				return {
					type: "sequential",
					pattern: sequentialPattern,
					start,
					padding,
				};
			case "case":
				return {
					type: "case_transform",
					mode: caseMode,
				};
			case "template":
				return {
					type: "template",
					template,
				};
		}
	}, [
		tab,
		find,
		replace,
		caseSensitive,
		sequentialPattern,
		start,
		padding,
		caseMode,
		template,
	]);

	// Reset hasLoadedOnce when dialog opens/closes
	useEffect(() => {
		if (!open) {
			hasLoadedOnce.current = false;
		}
	}, [open]);

	// Load preview whenever inputs change
	useEffect(() => {
		if (!open || paths.length === 0) return;

		const loadPreview = async () => {
			// Only show loading state on initial load, not when switching tabs
			if (!hasLoadedOnce.current) {
				setLoading(true);
			}
			try {
				const pattern = getPattern();
				const result = await invoke<RenamePreview[]>("preview_bulk_rename", {
					paths,
					pattern,
					autoRenameConflicts: autoRename,
				});
				setPreviews(result);
				hasLoadedOnce.current = true;
			} catch (e) {
				console.error("Preview error:", e);
				toast.error(String(e));
			} finally {
				setLoading(false);
			}
		};

		// Debounce preview updates
		const timeout = setTimeout(loadPreview, 300);
		return () => clearTimeout(timeout);
	}, [open, paths, autoRename, getPattern]);

	const handleApply = async () => {
		setExecuting(true);
		try {
			const pattern = getPattern();
			const result = await invoke<BulkRenameResult>("execute_bulk_rename", {
				paths,
				pattern,
				autoRenameConflicts: autoRename,
			});

			if (result.failed > 0) {
				toast.error(
					`Renamed ${result.success} files, ${result.failed} failed: ${result.errors.join(", ")}`,
				);
			} else {
				toast.success(`Successfully renamed ${result.success} files`);
			}

			onSuccess();
			onOpenChange(false);
		} catch (e) {
			console.error("Rename error:", e);
			toast.error(String(e));
		} finally {
			setExecuting(false);
		}
	};

	const hasConflicts = previews.some((p) => p.conflict);
	const hasErrors = previews.some((p) => p.error);
	const canApply = !loading && !executing && !hasConflicts && !hasErrors;

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="flex h-[85vh] max-w-4xl flex-col overflow-hidden">
				<DialogHeader>
					<DialogTitle>Bulk Rename ({paths.length} files)</DialogTitle>
					<DialogDescription>
						Choose a renaming pattern and preview changes before applying
					</DialogDescription>
				</DialogHeader>

				<Tabs
					value={tab}
					onValueChange={(v) => setTab(v as typeof tab)}
					className="flex min-h-0 flex-1 flex-col overflow-hidden"
				>
					<TabsList className="grid w-full grid-cols-4">
						<TabsTrigger value="find_replace">Find & Replace</TabsTrigger>
						<TabsTrigger value="sequential">Sequential</TabsTrigger>
						<TabsTrigger value="case">Case</TabsTrigger>
						<TabsTrigger value="template">Template</TabsTrigger>
					</TabsList>

					<div className="flex min-h-0 flex-1 flex-col gap-4 overflow-hidden pt-4">
						<TabsContent value="find_replace" className="mt-0">
							<div className="space-y-3">
								<div>
									<label htmlFor="find-input" className="font-medium text-sm">
										Find
									</label>
									<Input
										id="find-input"
										value={find}
										onChange={(e) => setFind(e.target.value)}
										placeholder="Text to find"
									/>
								</div>
								<div>
									<label
										htmlFor="replace-input"
										className="font-medium text-sm"
									>
										Replace with
									</label>
									<Input
										id="replace-input"
										value={replace}
										onChange={(e) => setReplace(e.target.value)}
										placeholder="Replacement text"
									/>
								</div>
								<div className="flex items-center space-x-2">
									<Checkbox
										checked={caseSensitive}
										onCheckedChange={(checked) =>
											setCaseSensitive(checked as boolean)
										}
										id="case-sensitive"
									/>
									<label
										htmlFor="case-sensitive"
										className="cursor-pointer text-sm"
									>
										Case sensitive
									</label>
								</div>
							</div>
						</TabsContent>

						<TabsContent value="sequential" className="mt-0">
							<div className="space-y-3">
								<div>
									<label
										htmlFor="pattern-input"
										className="font-medium text-sm"
									>
										Pattern
									</label>
									<Input
										id="pattern-input"
										value={sequentialPattern}
										onChange={(e) => setSequentialPattern(e.target.value)}
										placeholder="{name}_{n}"
									/>
									<p className="mt-1 text-[11px] text-muted-foreground">
										Use{" "}
										<code className="rounded bg-muted px-1 py-0.5">
											{"{n}"}
										</code>{" "}
										for number,{" "}
										<code className="rounded bg-muted px-1 py-0.5">
											{"{name}"}
										</code>{" "}
										for filename,{" "}
										<code className="rounded bg-muted px-1 py-0.5">
											{"{ext}"}
										</code>{" "}
										for extension
									</p>
								</div>
								<div className="grid grid-cols-2 gap-3">
									<div>
										<label
											htmlFor="start-input"
											className="font-medium text-sm"
										>
											Start number
										</label>
										<Input
											id="start-input"
											type="number"
											value={start}
											onChange={(e) => setStart(Number(e.target.value))}
											min={0}
										/>
									</div>
									<div>
										<label
											htmlFor="padding-input"
											className="font-medium text-sm"
										>
											Padding (zeros)
										</label>
										<Input
											id="padding-input"
											type="number"
											value={padding}
											onChange={(e) => setPadding(Number(e.target.value))}
											min={1}
											max={10}
										/>
									</div>
								</div>
							</div>
						</TabsContent>

						<TabsContent value="case" className="mt-0">
							<div className="space-y-2">
								<div className="font-medium text-sm">Transform to</div>
								<div className="space-y-2">
									{(["lowercase", "uppercase", "titlecase"] as const).map(
										(mode) => (
											<div key={mode} className="flex items-center space-x-2">
												<input
													type="radio"
													name="case-mode"
													id={mode}
													checked={caseMode === mode}
													onChange={() => setCaseMode(mode)}
													className="cursor-pointer"
												/>
												<label
													htmlFor={mode}
													className="cursor-pointer text-sm capitalize"
												>
													{mode.replace("case", " Case")}
												</label>
											</div>
										),
									)}
								</div>
							</div>
						</TabsContent>

						<TabsContent value="template" className="mt-0">
							<div className="space-y-3">
								<div>
									<label
										htmlFor="template-input"
										className="font-medium text-sm"
									>
										Template
									</label>
									<Input
										id="template-input"
										value={template}
										onChange={(e) => setTemplate(e.target.value)}
										placeholder="{name}_{date}"
									/>
								</div>
								<div className="space-y-1 rounded-md bg-muted p-3 text-xs">
									<p className="font-medium">Available variables:</p>
									<p>
										<code>{"{name}"}</code> - Original filename (no extension)
									</p>
									<p>
										<code>{"{ext}"}</code> - File extension
									</p>
									<p>
										<code>{"{date}"}</code> - Current date (YYYY-MM-DD)
									</p>
									<p>
										<code>{"{index}"}</code> - File index (0-based)
									</p>
									<p>
										<code>{"{counter}"}</code> - File counter (1-based)
									</p>
									<p>
										<code>{"{parent}"}</code> - Parent folder name
									</p>
									<p>
										<code>{"{type}"}</code> - File type (video/audio/file)
									</p>
									<p>
										<code>{"{video_codec}"}</code> - Video codec (media files
										only)
									</p>
									<p>
										<code>{"{audio_codec}"}</code> - Audio codec (media files
										only)
									</p>
								</div>
							</div>
						</TabsContent>

						{/* Preview section - takes remaining space */}
						<div className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-md border">
							<div className="flex shrink-0 items-center justify-between border-b bg-muted/40 p-3">
								<h3 className="font-medium text-sm">Preview</h3>
								<div className="flex items-center space-x-2">
									<Checkbox
										checked={autoRename}
										onCheckedChange={(checked) =>
											setAutoRename(checked as boolean)
										}
										id="auto-rename"
									/>
									<label
										htmlFor="auto-rename"
										className="cursor-pointer text-xs"
									>
										Auto-rename conflicts
									</label>
								</div>
							</div>
							<div className="min-h-0 flex-1 overflow-hidden">
								<ScrollArea className="h-full">
									<div className="space-y-1 p-2">
										{loading && (
											<div className="py-8 text-center text-muted-foreground text-sm">
												Generating preview...
											</div>
										)}
										{!loading && previews.length === 0 && (
											<div className="py-8 text-center text-muted-foreground text-sm">
												No files to rename
											</div>
										)}
										{!loading &&
											previews.map((preview) => (
												<div
													key={preview.original_path}
													className={cn(
														"flex items-center gap-2 rounded p-2 text-xs",
														preview.conflict && "bg-destructive/10",
														preview.error && "bg-destructive/10",
														!preview.conflict &&
															!preview.error &&
															"hover:bg-muted/50",
													)}
												>
													{preview.conflict || preview.error ? (
														<AlertCircle className="h-3.5 w-3.5 shrink-0 text-destructive" />
													) : (
														<Check className="h-3.5 w-3.5 shrink-0 text-green-600" />
													)}
													<div className="min-w-0 flex-1 space-y-0.5">
														<div className="flex flex-wrap items-center gap-2">
															<span className="break-all text-muted-foreground">
																{preview.original_name}
															</span>
															<X className="h-3 w-3 shrink-0" />
															<span
																className={cn(
																	"break-all font-medium",
																	(preview.conflict || preview.error) &&
																		"text-destructive",
																)}
															>
																{preview.new_name}
															</span>
														</div>
														{preview.error && (
															<div className="break-words text-[10px] text-destructive">
																{preview.error}
															</div>
														)}
														{preview.conflict && (
															<div className="text-[10px] text-destructive">
																Name conflict detected
															</div>
														)}
													</div>
												</div>
											))}
									</div>
								</ScrollArea>
							</div>
						</div>
					</div>
				</Tabs>

				<DialogFooter>
					{hasConflicts && !autoRename && (
						<p className="mr-auto text-destructive text-xs">
							Enable auto-rename to resolve conflicts
						</p>
					)}
					{hasErrors && (
						<p className="mr-auto text-destructive text-xs">
							Fix validation errors before applying
						</p>
					)}
					<Button
						variant="outline"
						onClick={() => onOpenChange(false)}
						disabled={executing}
					>
						Cancel
					</Button>
					<Button onClick={handleApply} disabled={!canApply}>
						{executing ? "Renaming..." : "Apply"}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
