import type { BitrateStatistics } from "@/types/bitrate";

interface BitrateStatisticsProps {
	statistics: BitrateStatistics;
}

// Format bitrate to human-readable string
function formatBitrate(bitrate: number): string {
	const mbps = bitrate / 1_000_000;
	const kbps = bitrate / 1_000;

	if (mbps >= 1) {
		return `${mbps.toFixed(2)} Mbps`;
	}
	return `${kbps.toFixed(2)} Kbps`;
}

// Format time as MM:SS
function formatTime(seconds: number): string {
	const mins = Math.floor(seconds / 60);
	const secs = Math.floor(seconds % 60);
	return `${mins}:${secs.toString().padStart(2, "0")}`;
}

export function BitrateStatisticsComponent({
	statistics,
}: BitrateStatisticsProps) {
	return (
		<div className="space-y-4">
			{/* Statistics Grid */}
			<div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
				<StatCard
					label="Min Bitrate"
					value={formatBitrate(statistics.min_bitrate)}
				/>
				<StatCard
					label="Max Bitrate"
					value={formatBitrate(statistics.max_bitrate)}
				/>
				<StatCard
					label="Avg Bitrate"
					value={formatBitrate(statistics.avg_bitrate)}
				/>
				<StatCard
					label="Median Bitrate"
					value={formatBitrate(statistics.median_bitrate)}
				/>
			</div>

			{/* Additional Stats */}
			<div className="grid grid-cols-2 gap-3">
				<StatCard
					label="Std Deviation"
					value={formatBitrate(Math.round(statistics.std_deviation))}
				/>
				<StatCard
					label="Total Data Points"
					value={statistics.total_frames.toString()}
				/>
			</div>

			{/* Peak Intervals */}
			{statistics.peak_intervals.length > 0 && (
				<div className="rounded-lg border border-border/50 bg-background p-4">
					<h4 className="mb-3 font-semibold text-sm">
						Peak Intervals ({statistics.peak_intervals.length})
					</h4>
					<div className="space-y-2">
						{statistics.peak_intervals.map((peak, idx) => (
							<div
								key={`${peak.start_time}-${peak.end_time}`}
								className="flex items-center justify-between rounded-md border border-border/50 bg-muted/20 p-3 text-xs"
							>
								<div>
									<span className="text-muted-foreground">
										Peak #{idx + 1}:{" "}
									</span>
									<span className="font-medium">
										{formatTime(peak.start_time)} - {formatTime(peak.end_time)}
									</span>
								</div>
								<div className="text-right">
									<div className="font-semibold text-primary">
										{formatBitrate(peak.peak_bitrate)}
									</div>
									<div className="text-muted-foreground">
										{peak.duration.toFixed(1)}s
									</div>
								</div>
							</div>
						))}
					</div>
				</div>
			)}
		</div>
	);
}

function StatCard({ label, value }: { label: string; value: string }) {
	return (
		<div className="rounded-lg border border-border/50 bg-muted/20 p-3">
			<div className="text-muted-foreground text-xs">{label}</div>
			<div className="mt-1 font-semibold text-foreground text-sm">{value}</div>
		</div>
	);
}
