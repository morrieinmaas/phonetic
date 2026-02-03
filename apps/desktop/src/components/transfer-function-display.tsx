import { useCallback, useEffect, useRef, useState } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import { useShallow } from "zustand/shallow";
import { useTraceStore } from "../stores/trace-store";
import type {
	ReferenceCurve,
	ReferenceCurveType,
	StoredTrace,
	TransferFunctionData,
} from "../types/audio";
import { ReferenceCurveSelector } from "./reference-curve-selector";
import { TraceControls } from "./trace-controls";

interface TransferFunctionDisplayProps {
	data: TransferFunctionData | null;
	isRunning: boolean;
	onResetAveraging: () => void;
}

/** Maximum number of stored traces that can be displayed simultaneously */
const MAX_DISPLAYED_TRACES = 4;

// Map to log frequency scale for display
const mapToLogScale = (
	linearData: number[],
	frequencies: number[],
	logFreqs: number[],
): number[] => {
	if (linearData.length === 0 || frequencies.length === 0) {
		return new Array(logFreqs.length).fill(-120);
	}

	return logFreqs.map((targetFreq) => {
		// Find closest frequency bin
		let closestIdx = 0;
		let minDiff = Math.abs((frequencies[0] ?? 0) - targetFreq);

		for (let i = 1; i < frequencies.length; i++) {
			const freq = frequencies[i];
			if (freq === undefined) continue;
			const diff = Math.abs(freq - targetFreq);
			if (diff < minDiff) {
				minDiff = diff;
				closestIdx = i;
			}
		}

		return linearData[closestIdx] ?? -120;
	});
};

// Generate logarithmic frequency points
const generateLogFrequencies = (minFreq: number, maxFreq: number, numPoints: number): number[] => {
	const logMin = Math.log10(minFreq);
	const logMax = Math.log10(maxFreq);
	const step = (logMax - logMin) / (numPoints - 1);

	return Array.from({ length: numPoints }, (_, i) => 10 ** (logMin + i * step));
};

// Calculate nice round scale bounds with padding
const calculateScaleBounds = (data: number[], padding = 0.1): [number, number] => {
	const validData = data.filter((v) => Number.isFinite(v) && v > -120);
	if (validData.length === 0) return [-40, 20];

	const min = Math.min(...validData);
	const max = Math.max(...validData);
	const range = max - min || 10;
	const paddedMin = min - range * padding;
	const paddedMax = max + range * padding;

	// Round to nice values (multiples of 5 or 10)
	const roundToNice = (val: number, roundUp: boolean): number => {
		const absVal = Math.abs(val);
		const magnitude = absVal < 10 ? 5 : 10;
		const rounded = roundUp
			? Math.ceil(val / magnitude) * magnitude
			: Math.floor(val / magnitude) * magnitude;
		return rounded;
	};

	return [roundToNice(paddedMin, false), roundToNice(paddedMax, true)];
};

export const TransferFunctionDisplay = ({
	data,
	isRunning,
	onResetAveraging,
}: TransferFunctionDisplayProps) => {
	const containerRef = useRef<HTMLDivElement>(null);
	const chartRef = useRef<uPlot | null>(null);
	const logFreqsRef = useRef<number[]>([]);
	const [autoZoom, setAutoZoom] = useState(true);
	const [theme, setTheme] = useState<"dark" | "light">(() =>
		document.documentElement.classList.contains("dark") ? "dark" : "light",
	);
	const [selectedRefCurve, setSelectedRefCurve] = useState<ReferenceCurveType | null>(null);
	const [refCurveData, setRefCurveData] = useState<ReferenceCurve | null>(null);

	// Get visible traces from store (limited to display count)
	// useShallow prevents infinite re-renders by comparing array contents shallowly
	const visibleTraces = useTraceStore(
		useShallow((state) => state.traces.filter((t) => t.visible).slice(0, MAX_DISPLAYED_TRACES)),
	);

	// Watch for theme changes
	useEffect(() => {
		const observer = new MutationObserver(() => {
			setTheme(document.documentElement.classList.contains("dark") ? "dark" : "light");
		});
		observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
		return () => observer.disconnect();
	}, []);

	// Initialize log frequencies
	useEffect(() => {
		logFreqsRef.current = generateLogFrequencies(20, 20000, 512);
	}, []);

	// Create chart (recreate when theme or number of visible traces changes)
	useEffect(() => {
		if (!containerRef.current) return;

		const container = containerRef.current;
		const width = container.clientWidth || 600;
		const height = Math.max(container.clientHeight, 250);

		// Theme-specific colors with high contrast
		const isDark = theme === "dark";
		const labelColor = isDark ? "#f5f5f5" : "#171717";
		const gridColor = isDark ? "rgba(255,255,255,0.2)" : "rgba(0,0,0,0.2)";
		const tickColor = isDark ? "rgba(255,255,255,0.4)" : "rgba(0,0,0,0.4)";

		// Build series array: [x, magnitude, phase, reference, ...traces]
		const series: uPlot.Series[] = [
			{}, // X axis
			{
				label: "Magnitude",
				stroke: isDark ? "oklch(0.75 0.18 280)" : "oklch(0.55 0.2 280)",
				width: 2.5,
				scale: "magnitude",
			},
			{
				label: "Phase",
				stroke: isDark ? "oklch(0.75 0.18 200)" : "oklch(0.55 0.2 200)",
				width: 1.5,
				scale: "phase",
				dash: [4, 2],
			},
			{
				label: "Reference",
				stroke: isDark ? "oklch(0.7 0.15 120)" : "oklch(0.5 0.2 120)",
				width: 1.5,
				scale: "magnitude",
				dash: [6, 3],
			},
		];

		// Add series for stored traces (pre-allocate slots for smooth updates)
		for (let i = 0; i < MAX_DISPLAYED_TRACES; i++) {
			const trace = visibleTraces[i];
			series.push({
				label: trace?.name ?? `Trace ${i + 1}`,
				stroke: trace?.color ?? "transparent",
				width: 2,
				scale: "magnitude",
				show: !!trace,
			});
		}

		const opts: uPlot.Options = {
			width,
			height,
			padding: [10, 20, 0, 0],
			cursor: {
				show: true,
				x: true,
				y: true,
			},
			scales: {
				x: {
					time: false,
					distr: 2,
					log: 10,
					min: 20,
					max: 20000,
				},
				magnitude: {
					auto: false,
					range: [-40, 20],
				},
				phase: {
					auto: false,
					range: [-180, 180],
				},
			},
			axes: [
				{
					label: "Frequency (Hz)",
					labelSize: 24,
					labelFont: "bold 13px system-ui, sans-serif",
					font: "bold 12px system-ui, sans-serif",
					stroke: labelColor,
					grid: { stroke: gridColor, width: 1 },
					ticks: { stroke: tickColor, width: 1, size: 6 },
					size: 45,
					gap: 8,
					splits: [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
					values: (_, ticks) =>
						ticks.map((v) => {
							if (v >= 1000) return `${(v / 1000).toFixed(0)}k`;
							return String(Math.round(v));
						}),
				},
				{
					label: "Magnitude (dB)",
					labelSize: 20,
					labelFont: "bold 13px system-ui, sans-serif",
					font: "bold 12px system-ui, sans-serif",
					stroke: labelColor,
					grid: { stroke: gridColor, width: 1 },
					ticks: { stroke: tickColor, width: 1, size: 6 },
					size: 55,
					gap: 8,
					scale: "magnitude",
				},
				{
					side: 1,
					label: "Phase (°)",
					labelSize: 20,
					labelFont: "bold 13px system-ui, sans-serif",
					font: "bold 12px system-ui, sans-serif",
					stroke: labelColor,
					grid: { show: false },
					ticks: { stroke: tickColor, width: 1, size: 6 },
					size: 55,
					gap: 8,
					scale: "phase",
					splits: [-180, -90, 0, 90, 180],
				},
			],
			series,
		};

		// Build initial data array
		const numPoints = logFreqsRef.current.length || 512;
		const chartData: uPlot.AlignedData = [
			logFreqsRef.current.length > 0 ? logFreqsRef.current : new Array(numPoints).fill(20),
			new Array(numPoints).fill(0), // Magnitude
			new Array(numPoints).fill(0), // Phase
			new Array(numPoints).fill(null), // Reference curve
		];

		// Add empty arrays for trace slots
		for (let i = 0; i < MAX_DISPLAYED_TRACES; i++) {
			chartData.push(new Array(numPoints).fill(null));
		}

		chartRef.current = new uPlot(opts, chartData, container);

		// Handle resize
		const resizeObserver = new ResizeObserver((entries) => {
			for (const entry of entries) {
				if (chartRef.current && entry.contentRect.width > 0) {
					chartRef.current.setSize({
						width: entry.contentRect.width,
						height: Math.max(entry.contentRect.height, 250),
					});
				}
			}
		});

		resizeObserver.observe(container);

		return () => {
			resizeObserver.disconnect();
			chartRef.current?.destroy();
			chartRef.current = null;
		};
	}, [theme, visibleTraces]);

	// Map reference curve to log frequency scale
	const getRefCurveValues = useCallback((): (number | null)[] => {
		if (!refCurveData || refCurveData.points.length === 0) {
			return new Array(logFreqsRef.current.length).fill(null);
		}

		return logFreqsRef.current.map((freq) => {
			// Find closest point in reference curve
			let closestIdx = 0;
			let minDiff = Math.abs(refCurveData.points[0]?.frequency ?? 0 - freq);

			for (let i = 1; i < refCurveData.points.length; i++) {
				const point = refCurveData.points[i];
				if (!point) continue;
				const diff = Math.abs(point.frequency - freq);
				if (diff < minDiff) {
					minDiff = diff;
					closestIdx = i;
				}
			}

			return refCurveData.points[closestIdx]?.amplitude_db ?? null;
		});
	}, [refCurveData]);

	// Map stored trace to log frequency scale
	const getTraceValues = useCallback((trace: StoredTrace): (number | null)[] => {
		return mapToLogScale(trace.data.magnitude_db, trace.data.frequencies, logFreqsRef.current);
	}, []);

	// Update chart data
	const updateChart = useCallback(() => {
		if (!chartRef.current) return;

		const logFreqs = logFreqsRef.current;
		const numPoints = logFreqs.length || 512;

		// Get live data or empty arrays
		const logMagnitude =
			data && isRunning
				? mapToLogScale(data.magnitude_db, data.frequencies, logFreqs)
				: new Array(numPoints).fill(null);
		const logPhase =
			data && isRunning
				? mapToLogScale(data.phase_deg, data.frequencies, logFreqs)
				: new Array(numPoints).fill(null);
		const refCurveValues = getRefCurveValues();

		// Build chart data array
		const chartData: uPlot.AlignedData = [
			logFreqs.length > 0 ? logFreqs : new Array(numPoints).fill(20),
			logMagnitude,
			logPhase,
			refCurveValues,
		];

		// Add trace data
		for (let i = 0; i < MAX_DISPLAYED_TRACES; i++) {
			const trace = visibleTraces[i];
			chartData.push(trace ? getTraceValues(trace) : new Array(numPoints).fill(null));
		}

		// Auto-zoom: calculate appropriate scale bounds considering all visible data
		const magnitudeScale = chartRef.current.scales.magnitude;
		if (magnitudeScale) {
			if (autoZoom) {
				// Combine all visible magnitude data for scale calculation
				const allMagnitudes = [
					...(data && isRunning ? logMagnitude : []),
					...visibleTraces.flatMap((t) => getTraceValues(t)),
				].filter((v): v is number => v !== null);

				if (allMagnitudes.length > 0) {
					const [magMin, magMax] = calculateScaleBounds(allMagnitudes);
					magnitudeScale.min = magMin;
					magnitudeScale.max = magMax;
				}
			} else {
				// Fixed scale
				magnitudeScale.min = -40;
				magnitudeScale.max = 20;
			}
		}

		chartRef.current.setData(chartData);
	}, [data, isRunning, autoZoom, getRefCurveValues, getTraceValues, visibleTraces]);

	useEffect(() => {
		updateChart();
	}, [updateChart]);

	// Calculate average coherence for display
	const avgCoherence =
		data && data.coherence.length > 0
			? data.coherence.reduce((a, b) => a + b, 0) / data.coherence.length
			: 0;

	return (
		<div className="flex flex-col h-full min-h-0">
			<div className="flex items-center justify-between mb-2 flex-shrink-0 flex-wrap gap-2">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Transfer Function
				</h2>
				<div className="flex items-center gap-3 flex-wrap">
					{isRunning && data && (
						<div className="flex items-center gap-2">
							<span className="text-xs text-muted-foreground">Coherence:</span>
							<div className="flex items-center gap-1">
								<div className="w-16 h-2 bg-muted rounded-full overflow-hidden">
									<div
										className={`h-full transition-all ${avgCoherence > 0.8 ? "bg-green-500" : avgCoherence > 0.5 ? "bg-yellow-500" : "bg-red-500"}`}
										style={{ width: `${avgCoherence * 100}%` }}
									/>
								</div>
								<span
									className={`text-xs font-mono w-8 ${avgCoherence > 0.8 ? "text-green-500" : avgCoherence > 0.5 ? "text-yellow-500" : "text-red-500"}`}
								>
									{(avgCoherence * 100).toFixed(0)}%
								</span>
							</div>
						</div>
					)}
					<TraceControls currentData={data} isRunning={isRunning} />
					<ReferenceCurveSelector
						selectedCurve={selectedRefCurve}
						onCurveChange={setSelectedRefCurve}
						onCurveDataChange={setRefCurveData}
						numPoints={512}
					/>
					<button
						type="button"
						onClick={() => setAutoZoom(!autoZoom)}
						className={`px-2 py-1 text-xs rounded border transition-colors ${
							autoZoom
								? "border-primary bg-primary/10 text-primary"
								: "border-border hover:bg-accent"
						}`}
					>
						Auto Zoom
					</button>
					<button
						type="button"
						onClick={onResetAveraging}
						disabled={!isRunning}
						className="px-2 py-1 text-xs rounded border border-border hover:bg-accent disabled:opacity-50 disabled:cursor-not-allowed"
					>
						Reset Avg
					</button>
				</div>
			</div>
			<div ref={containerRef} className="flex-1 min-h-[220px] relative">
				{!isRunning && visibleTraces.length === 0 && (
					<div className="absolute inset-0 flex items-center justify-center text-muted-foreground bg-background/50 z-10">
						Start audio stream and generator to measure transfer function
					</div>
				)}
			</div>
			<div className="flex gap-4 mt-2 text-xs text-muted-foreground flex-shrink-0 flex-wrap">
				<span className="flex items-center gap-1">
					<span className="w-4 h-0.5 bg-[oklch(0.7_0.15_280)]" />
					Live
				</span>
				<span className="flex items-center gap-1">
					<span
						className="w-4 h-0.5 bg-[oklch(0.7_0.15_200)] border-dashed"
						style={{ borderTopWidth: 1 }}
					/>
					Phase
				</span>
				{refCurveData && (
					<span className="flex items-center gap-1">
						<span
							className="w-4 h-0.5 bg-[oklch(0.6_0.15_120)] border-dashed"
							style={{ borderTopWidth: 1 }}
						/>
						{refCurveData.name}
					</span>
				)}
				{visibleTraces.map((trace) => (
					<span key={trace.id} className="flex items-center gap-1">
						<span className="w-4 h-0.5" style={{ backgroundColor: trace.color }} />
						{trace.name}
					</span>
				))}
			</div>
		</div>
	);
};
