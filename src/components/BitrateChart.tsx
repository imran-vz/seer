import { forwardRef, useImperativeHandle, useRef } from "react";
import {
	Area,
	AreaChart,
	CartesianGrid,
	ResponsiveContainer,
	Tooltip,
	XAxis,
	YAxis,
} from "recharts";
import { toast } from "sonner";
import type { BitrateDataPoint, StreamContribution } from "@/types/bitrate";

export interface BitrateChartProps {
	dataPoints: BitrateDataPoint[];
	duration: number;
	streamContributions?: StreamContribution[];
}

export interface BitrateChartHandle {
	exportToPng: () => Promise<void>;
}

// Theme-aware colors
interface ThemeColors {
	background: string;
	foreground: string;
	muted: string;
	border: string;
	axisColor: string;
}

const LIGHT_THEME: ThemeColors = {
	background: "#ffffff",
	foreground: "#0a0a0a",
	muted: "#737373",
	border: "#e5e5e5",
	axisColor: "#525252",
};

const DARK_THEME: ThemeColors = {
	background: "#0a0a0a",
	foreground: "#fafafa",
	muted: "#a3a3a3",
	border: "#262626",
	axisColor: "#888888",
};

// Stream type colors - using hex colors for PNG export compatibility
const STREAM_COLORS: Record<string, { stroke: string; fill: string }> = {
	video: {
		stroke: "#3b82f6", // blue-500
		fill: "rgba(59, 130, 246, 0.35)",
	},
	audio: {
		stroke: "#22c55e", // green-500
		fill: "rgba(34, 197, 94, 0.35)",
	},
	subtitle: {
		stroke: "#eab308", // yellow-500
		fill: "rgba(234, 179, 8, 0.35)",
	},
	unknown: {
		stroke: "#a855f7", // purple-500
		fill: "rgba(168, 85, 247, 0.35)",
	},
};

// Fallback combined color
const COMBINED_COLOR = {
	stroke: "#0ea5e9", // sky-500
	fill: "rgba(14, 165, 233, 0.4)",
};

// Format bitrate to human-readable string
function formatBitrate(bitrate: number): string {
	const mbps = bitrate / 1_000_000;
	const kbps = bitrate / 1_000;

	if (mbps >= 1) {
		return `${mbps.toFixed(2)} Mbps`;
	}
	return `${kbps.toFixed(0)} Kbps`;
}

// Format timestamp as MM:SS
function formatTime(seconds: number): string {
	const mins = Math.floor(seconds / 60);
	const secs = Math.floor(seconds % 60);
	return `${mins}:${secs.toString().padStart(2, "0")}`;
}

// Get display name for stream type
function getStreamTypeDisplay(type: string): string {
	const displayNames: Record<string, string> = {
		video: "Video",
		audio: "Audio",
		subtitle: "Subtitle",
		attachment: "Attachment",
		data: "Data",
		unknown: "Unknown",
	};
	return displayNames[type.toLowerCase()] || type;
}

// Get color for stream type
function getStreamColor(type: string): { stroke: string; fill: string } {
	return STREAM_COLORS[type.toLowerCase()] || STREAM_COLORS.unknown;
}

// Detect current theme from document
function getCurrentTheme(): "light" | "dark" {
	return document.documentElement.classList.contains("dark") ? "dark" : "light";
}

function getThemeColors(): ThemeColors {
	return getCurrentTheme() === "dark" ? DARK_THEME : LIGHT_THEME;
}

// Stream legend component
function StreamLegend({
	contributions,
}: {
	contributions: StreamContribution[];
}) {
	if (contributions.length === 0) return null;

	return (
		<div className="mt-3 flex flex-wrap items-center justify-center gap-4 border-border/30 border-t pt-3">
			{contributions.map((stream) => {
				const color = getStreamColor(stream.stream_type);
				return (
					<div
						key={stream.stream_index}
						className="flex items-center gap-2 text-xs"
					>
						<div
							className="h-3 w-3 rounded-sm"
							style={{ backgroundColor: color.stroke }}
						/>
						<span className="text-muted-foreground">
							<span className="font-medium text-foreground">
								{getStreamTypeDisplay(stream.stream_type)}
							</span>
							{stream.codec_name && (
								<span className="ml-1 opacity-70">({stream.codec_name})</span>
							)}
							<span className="ml-1 tabular-nums">
								{stream.percentage.toFixed(1)}%
							</span>
						</span>
					</div>
				);
			})}
		</div>
	);
}

// Merge stream data points into a single dataset for recharts
function mergeStreamData(
	combinedDataPoints: BitrateDataPoint[],
	streamContributions: StreamContribution[],
): Array<Record<string, number>> {
	// Create a map of timestamp -> data
	const dataMap = new Map<number, Record<string, number>>();

	// Initialize with combined data points
	for (const point of combinedDataPoints) {
		dataMap.set(point.timestamp, {
			timestamp: point.timestamp,
			combined: point.bitrate,
		});
	}

	// Add per-stream data
	for (const stream of streamContributions) {
		const streamKey = `stream_${stream.stream_index}`;
		for (const point of stream.data_points) {
			const existing = dataMap.get(point.timestamp);
			if (existing) {
				existing[streamKey] = point.bitrate;
			} else {
				dataMap.set(point.timestamp, {
					timestamp: point.timestamp,
					combined: 0,
					[streamKey]: point.bitrate,
				});
			}
		}
	}

	// Sort by timestamp and return as array
	return Array.from(dataMap.values()).sort((a, b) => a.timestamp - b.timestamp);
}

/**
 * Get all computed styles for an element as inline style string
 */
function getComputedStylesAsString(element: Element): string {
	const computed = getComputedStyle(element);
	const styles: string[] = [];

	// Key properties that affect rendering
	const importantProps = [
		"font-family",
		"font-size",
		"font-weight",
		"fill",
		"stroke",
		"stroke-width",
		"stroke-dasharray",
		"opacity",
		"color",
		"background-color",
		"border-color",
		"text-anchor",
		"dominant-baseline",
	];

	for (const prop of importantProps) {
		const value = computed.getPropertyValue(prop);
		if (value && value !== "none" && value !== "") {
			styles.push(`${prop}: ${value}`);
		}
	}

	return styles.join("; ");
}

/**
 * Clone element and inline all styles recursively
 */
function cloneWithInlineStyles(element: Element): Element {
	const clone = element.cloneNode(false) as Element;

	// Copy attributes
	for (const attr of Array.from(element.attributes)) {
		clone.setAttribute(attr.name, attr.value);
	}

	// Add computed styles as inline style
	const styleString = getComputedStylesAsString(element);
	if (styleString) {
		const existingStyle = clone.getAttribute("style") || "";
		clone.setAttribute(
			"style",
			existingStyle ? `${existingStyle}; ${styleString}` : styleString,
		);
	}

	// Handle text nodes
	for (const child of Array.from(element.childNodes)) {
		if (child.nodeType === Node.TEXT_NODE) {
			clone.appendChild(child.cloneNode(true));
		} else if (child.nodeType === Node.ELEMENT_NODE) {
			clone.appendChild(cloneWithInlineStyles(child as Element));
		}
	}

	return clone;
}

/**
 * Export the chart container to PNG using canvas
 */
async function exportToPng(
	container: HTMLElement,
	filename: string,
	streamContributions: StreamContribution[],
): Promise<void> {
	// Get current theme colors
	const theme = getThemeColors();

	// Get the dimensions
	const rect = container.getBoundingClientRect();
	const width = rect.width;
	const height = rect.height;

	// Create a canvas
	const scale = 2; // 2x for retina
	const canvas = document.createElement("canvas");
	canvas.width = width * scale;
	canvas.height = height * scale;

	const ctx = canvas.getContext("2d");
	if (!ctx) {
		throw new Error("Could not get canvas context");
	}

	// Fill background with theme color
	ctx.fillStyle = theme.background;
	ctx.fillRect(0, 0, canvas.width, canvas.height);

	// Scale for retina
	ctx.scale(scale, scale);

	// Find the SVG element
	const svg = container.querySelector("svg");
	if (svg) {
		// Clone SVG with inlined styles
		const svgClone = cloneWithInlineStyles(svg) as SVGElement;

		// Set explicit dimensions
		svgClone.setAttribute("width", String(svg.getBoundingClientRect().width));
		svgClone.setAttribute("height", String(svg.getBoundingClientRect().height));

		// Remove any CSS class references since styles are inlined
		svgClone.removeAttribute("class");

		// Replace currentColor and CSS variables with actual colors
		const svgElements = svgClone.querySelectorAll("*");
		for (const el of Array.from(svgElements)) {
			const svgEl = el as SVGElement;

			// Fix stroke
			const stroke = svgEl.getAttribute("stroke");
			if (stroke === "currentColor" || stroke?.includes("currentcolor")) {
				svgEl.setAttribute("stroke", theme.axisColor);
			}

			// Fix fill
			const fill = svgEl.getAttribute("fill");
			if (fill === "currentColor" || fill?.includes("currentcolor")) {
				svgEl.setAttribute("fill", theme.axisColor);
			}

			// Fix style attribute
			const style = svgEl.getAttribute("style");
			if (style) {
				const newStyle = style
					.replace(/currentColor/gi, theme.axisColor)
					.replace(/currentcolor/gi, theme.axisColor);
				svgEl.setAttribute("style", newStyle);
			}
		}

		// Serialize to string
		const serializer = new XMLSerializer();
		let svgString = serializer.serializeToString(svgClone);

		// Ensure proper XML namespace
		if (!svgString.includes("xmlns=")) {
			svgString = svgString.replace(
				"<svg",
				'<svg xmlns="http://www.w3.org/2000/svg"',
			);
		}

		// Create blob and image
		const svgBlob = new Blob([svgString], {
			type: "image/svg+xml;charset=utf-8",
		});
		const svgUrl = URL.createObjectURL(svgBlob);

		const img = new Image();
		await new Promise<void>((resolve, reject) => {
			img.onload = () => resolve();
			img.onerror = (e) => reject(new Error(`Failed to load SVG: ${e}`));
			img.src = svgUrl;
		});

		// Draw SVG
		const svgRect = svg.getBoundingClientRect();
		const svgX = svgRect.left - rect.left;
		const svgY = svgRect.top - rect.top;
		ctx.drawImage(img, svgX, svgY);

		URL.revokeObjectURL(svgUrl);
	}

	// Draw legends
	const legend = container.querySelector(".border-t");
	if (legend && streamContributions.length > 0) {
		const legendRect = legend.getBoundingClientRect();
		const legendY = legendRect.top - rect.top;

		ctx.font = "12px system-ui, -apple-system, sans-serif";
		ctx.textBaseline = "middle";

		// Calculate total width needed for centering
		let totalWidth = 0;
		const itemWidths: number[] = [];
		for (const stream of streamContributions) {
			const typeDisplay = getStreamTypeDisplay(stream.stream_type);
			const codecText = stream.codec_name ? ` (${stream.codec_name})` : "";
			const percentText = ` ${stream.percentage.toFixed(1)}%`;
			const fullText = typeDisplay + codecText + percentText;
			const itemWidth = 12 + 8 + ctx.measureText(fullText).width + 24; // box + gap + text + spacing
			itemWidths.push(itemWidth);
			totalWidth += itemWidth;
		}

		// Start from center
		let xOffset = (width - totalWidth) / 2;

		for (let i = 0; i < streamContributions.length; i++) {
			const stream = streamContributions[i];
			const color = getStreamColor(stream.stream_type);

			// Draw color box
			ctx.fillStyle = color.stroke;
			ctx.fillRect(xOffset, legendY + 6, 12, 12);

			// Draw text
			let textX = xOffset + 20;
			const textY = legendY + 12;

			// Stream type (bold/foreground)
			ctx.fillStyle = theme.foreground;
			ctx.font = "600 12px system-ui, -apple-system, sans-serif";
			const typeText = getStreamTypeDisplay(stream.stream_type);
			ctx.fillText(typeText, textX, textY);
			textX += ctx.measureText(typeText).width;

			// Codec (muted)
			ctx.fillStyle = theme.muted;
			ctx.font = "12px system-ui, -apple-system, sans-serif";
			if (stream.codec_name) {
				const codecText = ` (${stream.codec_name})`;
				ctx.fillText(codecText, textX, textY);
				textX += ctx.measureText(codecText).width;
			}

			// Percentage (muted)
			const percentText = ` ${stream.percentage.toFixed(1)}%`;
			ctx.fillText(percentText, textX, textY);

			xOffset += itemWidths[i];
		}
	}

	// Download
	canvas.toBlob((blob) => {
		if (!blob) {
			console.error("Failed to create PNG blob");
			return;
		}

		const url = URL.createObjectURL(blob);
		const a = document.createElement("a");
		a.href = url;
		a.download = filename;
		document.body.appendChild(a);
		a.click();
		document.body.removeChild(a);

		toast.success(`Exported ${filename}`);
		URL.revokeObjectURL(url);
	}, "image/png");
}

export const BitrateChart = forwardRef<BitrateChartHandle, BitrateChartProps>(
	function BitrateChart(
		{ dataPoints, duration, streamContributions = [] },
		ref,
	) {
		const containerRef = useRef<HTMLDivElement>(null);

		// Expose export function to parent
		useImperativeHandle(
			ref,
			() => ({
				exportToPng: async () => {
					if (!containerRef.current) {
						console.error("Chart container not found");
						return;
					}

					const timestamp = new Date()
						.toISOString()
						.slice(0, 19)
						.replace(/:/g, "-");

					const theme = getCurrentTheme();

					try {
						await exportToPng(
							containerRef.current,
							`bitrate-chart-${theme}-${timestamp}.png`,
							streamContributions,
						);
					} catch (error) {
						console.error("Failed to export chart:", error);
					}
				},
			}),
			[streamContributions],
		);

		if (dataPoints.length === 0) {
			return (
				<div className="flex h-75 items-center justify-center rounded-lg border border-border/50 bg-muted/20">
					<p className="text-muted-foreground text-sm">
						No data points available
					</p>
				</div>
			);
		}

		// Check if we have per-stream data
		const hasStreamData =
			streamContributions.length > 0 &&
			streamContributions.some(
				(s) => s.data_points && s.data_points.length > 0,
			);

		// Merge data for multi-stream view
		const chartData = hasStreamData
			? mergeStreamData(dataPoints, streamContributions)
			: dataPoints;

		// Get axis color based on theme (for display, CSS handles this)
		const axisColor = "currentColor";

		return (
			<div
				ref={containerRef}
				className="rounded-lg border border-border/50 bg-background p-4"
			>
				<ResponsiveContainer width="100%" height={300}>
					<AreaChart data={chartData}>
						<defs>
							{/* Combined gradient */}
							<linearGradient id="combinedGradient" x1="0" y1="0" x2="0" y2="1">
								<stop offset="5%" stopColor={COMBINED_COLOR.fill} />
								<stop offset="95%" stopColor="rgba(14, 165, 233, 0.05)" />
							</linearGradient>
							{/* Per-stream gradients */}
							{streamContributions.map((stream) => {
								const color = getStreamColor(stream.stream_type);
								const baseColor = color.stroke;
								return (
									<linearGradient
										key={`gradient_${stream.stream_index}`}
										id={`gradient_stream_${stream.stream_index}`}
										x1="0"
										y1="0"
										x2="0"
										y2="1"
									>
										<stop offset="5%" stopColor={color.fill} />
										<stop
											offset="95%"
											stopColor={`${baseColor}0D`} // 5% opacity
										/>
									</linearGradient>
								);
							})}
						</defs>
						<CartesianGrid
							strokeDasharray="3 3"
							stroke={axisColor}
							opacity={0.3}
						/>
						<XAxis
							dataKey="timestamp"
							type="number"
							domain={[0, duration]}
							tickFormatter={formatTime}
							stroke={axisColor}
							fontSize={11}
							tickLine={{ stroke: axisColor, opacity: 0.5 }}
							axisLine={{ stroke: axisColor, opacity: 0.5 }}
						/>
						<YAxis
							tickFormatter={(value) => {
								const mbps = value / 1_000_000;
								return mbps >= 1
									? `${mbps.toFixed(1)}M`
									: `${(value / 1_000).toFixed(0)}K`;
							}}
							stroke={axisColor}
							fontSize={11}
							tickLine={{ stroke: axisColor, opacity: 0.5 }}
							axisLine={{ stroke: axisColor, opacity: 0.5 }}
						/>
						<Tooltip
							contentStyle={{
								backgroundColor: "var(--popover)",
								border: "1px solid var(--border)",
								borderRadius: "0.5rem",
								fontSize: "0.75rem",
								color: "var(--popover-foreground)",
							}}
							labelStyle={{
								color: "var(--muted-foreground)",
								marginBottom: "0.25rem",
							}}
							labelFormatter={(value) => `Time: ${formatTime(Number(value))}`}
							formatter={(value: number | undefined, name?: string) => {
								// Find stream info for better labeling
								if (name?.startsWith("stream_")) {
									const streamIndex = Number.parseInt(
										name.replace("stream_", ""),
										10,
									);
									const stream = streamContributions.find(
										(s) => s.stream_index === streamIndex,
									);
									if (stream) {
										const label = `${getStreamTypeDisplay(stream.stream_type)}${stream.codec_name ? ` (${stream.codec_name})` : ""}`;
										return [formatBitrate(value || 0), label];
									}
								}
								return [formatBitrate(value || 0), "Combined"];
							}}
						/>

						{/* Render per-stream areas if available */}
						{hasStreamData ? (
							streamContributions.map((stream) => {
								const color = getStreamColor(stream.stream_type);
								return (
									<Area
										key={`area_${stream.stream_index}`}
										type="monotone"
										dataKey={`stream_${stream.stream_index}`}
										stroke={color.stroke}
										strokeWidth={2}
										fill={`url(#gradient_stream_${stream.stream_index})`}
										animationDuration={300}
										name={`stream_${stream.stream_index}`}
									/>
								);
							})
						) : (
							/* Fallback to combined view */
							<Area
								type="monotone"
								dataKey="bitrate"
								stroke={COMBINED_COLOR.stroke}
								strokeWidth={2}
								fill="url(#combinedGradient)"
								animationDuration={300}
								name="combined"
							/>
						)}
					</AreaChart>
				</ResponsiveContainer>

				{/* Stream contributions legend */}
				{streamContributions.length > 0 && (
					<StreamLegend contributions={streamContributions} />
				)}
			</div>
		);
	},
);
