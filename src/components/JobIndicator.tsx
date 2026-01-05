import { Clock, Loader2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { useBitrateStore } from "@/stores/bitrateStore";
import { useFileBrowserStore } from "@/stores/fileBrowserStore";
import type { JobInfo } from "@/types/bitrate";

interface DisplayJob extends JobInfo {
	fileName: string;
	queuePosition?: number;
}

export function JobIndicator() {
	const { queueStatus, cancelAnalysis, cancelAllJobs } = useBitrateStore();
	const navigateToFile = useFileBrowserStore((state) => state.navigateToFile);

	if (!queueStatus) {
		return null;
	}

	const { queued, running } = queueStatus;
	const totalJobs = queued.length + running.length;

	// Don't render if no jobs
	if (totalJobs === 0) {
		return null;
	}

	// Prepare display jobs with file names and queue positions
	const displayJobs: DisplayJob[] = [
		...running.map((job) => ({
			...job,
			fileName: job.path.split("/").pop() || job.path,
		})),
		...queued.map((job, index) => ({
			...job,
			fileName: job.path.split("/").pop() || job.path,
			queuePosition: index + 1,
		})),
	];

	const handleCancelJob = async (job: DisplayJob) => {
		await cancelAnalysis(job.path);
	};

	const handleJobClick = async (job: DisplayJob) => {
		await navigateToFile(job.path);
	};

	const handleCancelAll = async () => {
		await cancelAllJobs();
	};

	return (
		<DropdownMenu>
			<DropdownMenuTrigger asChild>
				<Button
					variant="ghost"
					size="icon-sm"
					className="relative shrink-0"
					title={`${totalJobs} job${totalJobs > 1 ? "s" : ""} (${running.length} running, ${queued.length} queued)`}
				>
					<Loader2 className="h-4 w-4 animate-spin" />
					{totalJobs > 1 && (
						<span className="absolute -top-0.5 -right-0.5 flex h-3.5 w-3.5 items-center justify-center rounded-full bg-primary text-[10px] text-primary-foreground">
							{totalJobs}
						</span>
					)}
				</Button>
			</DropdownMenuTrigger>
			<DropdownMenuContent align="end" className="w-72">
				<DropdownMenuLabel className="flex items-center justify-between gap-2">
					<div className="flex items-center gap-2">
						<Loader2 className="h-3 w-3 animate-spin" />
						Jobs ({running.length} running, {queued.length} queued)
					</div>
					{totalJobs > 1 && (
						<Button
							variant="ghost"
							size="sm"
							className="h-6 px-2 text-destructive text-xs hover:text-destructive"
							onClick={handleCancelAll}
						>
							Cancel All
						</Button>
					)}
				</DropdownMenuLabel>
				<DropdownMenuSeparator />
				{displayJobs.map((job) => (
					<DropdownMenuItem
						key={job.path}
						className="flex cursor-pointer items-center justify-between gap-2"
						onSelect={() => handleJobClick(job)}
					>
						<div className="flex min-w-0 flex-1 flex-col gap-0.5">
							<span className="truncate font-medium text-xs">
								{job.fileName}
							</span>
							<div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
								{job.state === "queued" ? (
									<>
										<Clock className="h-2.5 w-2.5" />
										<span>Queued - position {job.queuePosition}</span>
									</>
								) : (
									<>
										<Loader2 className="h-2.5 w-2.5 animate-spin" />
										<span>Running</span>
									</>
								)}
							</div>
						</div>
						<Button
							variant="ghost"
							size="icon-sm"
							className="h-6 w-6 shrink-0 text-muted-foreground hover:text-destructive"
							onClick={(e) => {
								e.stopPropagation();
								handleCancelJob(job);
							}}
							title="Cancel job"
						>
							<X className="h-3 w-3" />
						</Button>
					</DropdownMenuItem>
				))}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
