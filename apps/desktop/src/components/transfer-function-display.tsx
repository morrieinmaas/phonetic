import type { MouseEvent as ReactMouseEvent } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import { useShallow } from "zustand/shallow";
import { interpolatePhaseToLogScale } from "../lib/phase-interpolation";
import {
	type CursorSample,
	applyPan,
	applyZoom,
	nearestIndex,
	pixelToLogFrequency,
} from "../lib/plot-scales";
import { audioApi } from "../services/audio-api";
import { useEqStore } from "../stores/eq-store";
import { useTraceStore } from "../stores/trace-store";
import type {
	BiquadFilter,
	ReferenceCurve,
	ReferenceCurveType,
	StoredTrace,
	TransferFunctionData,
} from "../types/audio";
import { CursorReadout } from "./cursor-readout";
import { EqControls } from "./eq-controls";
import { ReferenceCurveSelector } from "./reference-curve-selector";
import { TraceControls } from "./trace-controls";

interface TransferFunctionDisplayProps {
	data: TransferFunctionData | null;
	isRunning: boolean;
	onResetAveraging: () => void;
	/** Sample rate the virtual EQ's filter coefficients are computed against. */
	sampleRate: number;
}

/** Maximum number of stored traces that can be displayed simultaneously */
const MAX_DISPLAYED_TRACES = 4;

/** Full frequency range of the plot, and the limit that zoom/pan gestures cannot cross. */
const FREQ_LIMITS: [number, number] = [20, 20000];

const CURSOR_A_COLOR = "#f59e0b";
const CURSOR_B_COLOR = "#d946ef";

/** How often the virtual-EQ prediction may be recomputed, in milliseconds.
    A predicted curve is a reference overlay, not the live trace; recomputing
    it per measurement frame cost an IPC round trip of the whole transfer
    function each time. */
const EQ_PREDICTION_INTERVAL_MS = 250;

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
	sampleRate,
}: TransferFunctionDisplayProps) => {
	const containerRef = useRef<HTMLDivElement>(null);
	const plotHostRef = useRef<HTMLDivElement>(null);
	const chartRef = useRef<uPlot | null>(null);
	const logFreqsRef = useRef<number[]>([]);
	const xRangeRef = useRef<[number, number]>(FREQ_LIMITS);
	const cursorsRef = useRef<{ a: CursorSample | null; b: CursorSample | null }>({
		a: null,
		b: null,
	});
	const [cursorA, setCursorA] = useState<CursorSample | null>(null);
	const [cursorB, setCursorB] = useState<CursorSample | null>(null);
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

	// Virtual EQ: predict the effect of the enabled filter bank on the live measurement,
	// or on the first visible stored trace when nothing is running.
	const enabledEqFilters = useEqStore(
		useShallow((state) => state.filters.filter((f) => f.enabled)),
	);
	const eqBypassed = useEqStore((state) => state.bypassed);
	const [predictedData, setPredictedData] = useState<TransferFunctionData | null>(null);
	const lastEqPredictionRef = useRef(0);
	const eqSourceData = data ?? visibleTraces[0]?.data ?? null;
	const eqActive = !eqBypassed && enabledEqFilters.length > 0 && eqSourceData !== null;

	// The EQ prediction is an overlay on a live trace, so eqSourceData changes on
	// every measurement poll. Running it per frame meant an IPC round trip
	// serialising the whole transfer function both ways, sixty times a second,
	// each one contending for the processor lock — enough to stall the app
	// whenever a filter was enabled. An operator cannot read a predicted curve
	// faster than a few updates a second, so it is throttled to that.
	useEffect(() => {
		if (!eqActive || !eqSourceData) {
			setPredictedData(null);
			return;
		}

		let cancelled = false;
		const filterBank: BiquadFilter[] = enabledEqFilters.map((f) => ({
			filter_type: f.filter_type,
			frequency_hz: f.frequency_hz,
			gain_db: f.gain_db,
			q: f.q,
			enabled: f.enabled,
		}));

		const sinceLast = Date.now() - lastEqPredictionRef.current;
		const delay = Math.max(0, EQ_PREDICTION_INTERVAL_MS - sinceLast);

		const timer = setTimeout(() => {
			lastEqPredictionRef.current = Date.now();
			audioApi
				.predictEq(eqSourceData, filterBank, sampleRate)
				.then((result) => {
					if (!cancelled) setPredictedData(result);
				})
				.catch(() => {
					if (!cancelled) setPredictedData(null);
				});
		}, delay);

		return () => {
			cancelled = true;
			clearTimeout(timer);
		};
	}, [eqActive, eqSourceData, enabledEqFilters, sampleRate]);

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
		// Follow the container. A floor here would draw past the panel edge.
		const height = Math.max(container.clientHeight, 120);

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
			{
				label: "Predicted (EQ)",
				stroke: isDark ? "oklch(0.75 0.18 30)" : "oklch(0.55 0.2 30)",
				width: 2,
				scale: "magnitude",
				dash: [2, 3],
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
					distr: 3,
					log: 10,
					min: xRangeRef.current[0],
					max: xRangeRef.current[1],
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
					labelSize: 22,
					labelFont: "bold 13px system-ui, sans-serif",
					font: "bold 12px system-ui, sans-serif",
					stroke: labelColor,
					grid: { stroke: gridColor, width: 1 },
					ticks: { stroke: tickColor, width: 1, size: 6 },
					// Gutter must hold the rotated title (22), a four-glyph value
					// like -120 at bold 12px (~34), the gap (8) and the tick (6).
					// At 55 the title was clipped to "agnitude".
					size: 72,
					gap: 8,
					scale: "magnitude",
					splits: [-40, -30, -20, -10, 0, 10, 20],
				},
				{
					side: 1,
					label: "Phase (°)",
					labelSize: 22,
					labelFont: "bold 13px system-ui, sans-serif",
					font: "bold 12px system-ui, sans-serif",
					stroke: labelColor,
					grid: { show: false },
					ticks: { stroke: tickColor, width: 1, size: 6 },
					size: 72,
					gap: 8,
					scale: "phase",
					splits: [-180, -90, 0, 90, 180],
				},
			],
			series,
			hooks: {
				draw: [
					(u) => {
						const drawCursorLine = (sample: CursorSample | null, color: string) => {
							if (!sample) return;
							const x = u.valToPos(sample.frequencyHz, "x", true);
							if (x < u.bbox.left || x > u.bbox.left + u.bbox.width) return;

							u.ctx.save();
							u.ctx.strokeStyle = color;
							u.ctx.lineWidth = 1;
							u.ctx.setLineDash([4, 3]);
							u.ctx.beginPath();
							u.ctx.moveTo(x, u.bbox.top);
							u.ctx.lineTo(x, u.bbox.top + u.bbox.height);
							u.ctx.stroke();
							u.ctx.restore();
						};

						drawCursorLine(cursorsRef.current.a, CURSOR_A_COLOR);
						drawCursorLine(cursorsRef.current.b, CURSOR_B_COLOR);
					},
				],
			},
		};

		// Build initial data array
		const numPoints = logFreqsRef.current.length || 512;
		const chartData: uPlot.AlignedData = [
			logFreqsRef.current.length > 0 ? logFreqsRef.current : new Array(numPoints).fill(20),
			new Array(numPoints).fill(0), // Magnitude
			new Array(numPoints).fill(0), // Phase
			new Array(numPoints).fill(null), // Reference curve
			new Array(numPoints).fill(null), // Predicted (EQ)
		];

		// Add empty arrays for trace slots
		for (let i = 0; i < MAX_DISPLAYED_TRACES; i++) {
			chartData.push(new Array(numPoints).fill(null));
		}

		const host = plotHostRef.current;
		if (!host) return;
		chartRef.current = new uPlot(opts, chartData, host);

		// Handle resize
		// Batched into a frame so a burst of resizes coalesces into one setSize,
		// and guarded so a zero-sized container (hidden tab, collapsed panel)
		// never produces a degenerate canvas.
		let resizeFrame = 0;
		const resizeObserver = new ResizeObserver((entries) => {
			const entry = entries[entries.length - 1];
			if (!entry) return;
			const { width, height } = entry.contentRect;
			if (width <= 0 || height <= 0) return;
			cancelAnimationFrame(resizeFrame);
			resizeFrame = requestAnimationFrame(() => {
				chartRef.current?.setSize({ width, height });
			});
		});

		resizeObserver.observe(container);

		return () => {
			cancelAnimationFrame(resizeFrame);
			resizeObserver.disconnect();
			chartRef.current?.destroy();
			chartRef.current = null;
		};
	}, [theme, visibleTraces]);

	// Keep the cursor draw hook's ref in sync and trigger a redraw without rebuilding the chart.
	useEffect(() => {
		cursorsRef.current = { a: cursorA, b: cursorB };
		chartRef.current?.redraw();
	}, [cursorA, cursorB]);

	// Trackpad gestures: ctrl+wheel pinch zooms about the pointer, plain wheel pans.
	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		const handleWheel = (event: WheelEvent) => {
			if (!chartRef.current) return;
			event.preventDefault();

			const rect = container.getBoundingClientRect();
			const width = rect.width || 1;
			const pointerX = event.clientX - rect.left;
			const [lo, hi] = xRangeRef.current;
			const anchor = pixelToLogFrequency(pointerX, width, lo, hi);

			xRangeRef.current = event.ctrlKey
				? applyZoom(xRangeRef.current, Math.exp(event.deltaY * 0.01), anchor, FREQ_LIMITS)
				: applyPan(xRangeRef.current, (event.deltaX || event.deltaY) / width, FREQ_LIMITS);

			chartRef.current.setScale("x", { min: xRangeRef.current[0], max: xRangeRef.current[1] });
		};

		container.addEventListener("wheel", handleWheel, { passive: false });
		return () => container.removeEventListener("wheel", handleWheel);
	}, []);

	// Place cursor A on click, cursor B on shift-click, by sampling the nearest measured point.
	const handlePlotClick = useCallback(
		(event: ReactMouseEvent<HTMLDivElement>) => {
			if (!data || !isRunning || !containerRef.current) return;

			const rect = containerRef.current.getBoundingClientRect();
			const width = rect.width || 1;
			const x = event.clientX - rect.left;
			const [lo, hi] = xRangeRef.current;
			const freq = pixelToLogFrequency(x, width, lo, hi);

			const idx = nearestIndex(data.frequencies, freq);
			if (idx === -1) return;

			const sample: CursorSample = {
				frequencyHz: data.frequencies[idx] ?? freq,
				magnitudeDb: data.magnitude_db[idx] ?? 0,
				groupDelayMs: data.group_delay_ms[idx] ?? 0,
			};

			if (event.shiftKey) {
				setCursorB(sample);
			} else {
				setCursorA(sample);
			}
		},
		[data, isRunning],
	);

	// Double-click resets the plot back to the full frequency range.
	const handleResetZoom = useCallback(() => {
		xRangeRef.current = FREQ_LIMITS;
		chartRef.current?.setScale("x", { min: FREQ_LIMITS[0], max: FREQ_LIMITS[1] });
	}, []);

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
				? interpolatePhaseToLogScale(data.phase_deg, data.frequencies, logFreqs, mapToLogScale)
				: new Array(numPoints).fill(null);
		const refCurveValues = getRefCurveValues();
		const predictedValues =
			eqActive && predictedData
				? mapToLogScale(predictedData.magnitude_db, predictedData.frequencies, logFreqs)
				: new Array(numPoints).fill(null);

		// Build chart data array
		const chartData: uPlot.AlignedData = [
			logFreqs.length > 0 ? logFreqs : new Array(numPoints).fill(20),
			logMagnitude,
			logPhase,
			refCurveValues,
			predictedValues,
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
					...predictedValues,
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
	}, [
		data,
		isRunning,
		autoZoom,
		getRefCurveValues,
		getTraceValues,
		visibleTraces,
		eqActive,
		predictedData,
	]);

	useEffect(() => {
		updateChart();
	}, [updateChart]);

	// Calculate average coherence for display
	const avgCoherence =
		data && data.coherence.length > 0
			? data.coherence.reduce((a, b) => a + b, 0) / data.coherence.length
			: 0;

	return (
		<div className="flex h-full min-h-0 flex-col gap-2">
			{/* Identity and live measurement quality */}
			<div className="flex flex-shrink-0 items-center justify-between gap-3">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Transfer Function
				</h2>
				<div className="flex items-center gap-3">
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
				</div>
			</div>

			{/* Control bar: capture and trace actions on the left, view options on
			    the right, so the two kinds of control stay visually distinct. */}
			<div className="flex flex-shrink-0 flex-wrap items-center gap-x-3 gap-y-2 rounded-md border border-border/60 bg-muted/30 px-2 py-1.5">
				<TraceControls currentData={data} isRunning={isRunning} />
				<div className="ml-auto flex items-center gap-2">
					<ReferenceCurveSelector
						selectedCurve={selectedRefCurve}
						onCurveChange={setSelectedRefCurve}
						onCurveDataChange={setRefCurveData}
						numPoints={512}
					/>
					<button
						type="button"
						aria-pressed={autoZoom}
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
			<details className="flex-shrink-0 rounded-md border border-border/60 bg-muted/20">
				<summary className="cursor-pointer select-none px-2 py-1.5 text-xs font-semibold uppercase tracking-wider text-muted-foreground marker:text-muted-foreground">
					Virtual EQ
					{eqActive && <span className="ml-2 normal-case text-primary">· predicting</span>}
				</summary>
				<div className="border-t border-border/60 p-2">
					<EqControls />
				</div>
			</details>
			{/* biome-ignore lint/a11y/useKeyWithClickEvents: pointer-only cursor placement on a continuous log-frequency plot has no meaningful keyboard equivalent */}
			<div
				ref={containerRef}
				data-testid="transfer-function-plot"
				className="relative min-h-0 flex-1 overflow-hidden"
				onClick={handlePlotClick}
				onDoubleClick={handleResetZoom}
			>
				{/* uPlot mounts here, out of flow, so the canvas can never change the
				    size of the container being observed. In flow the two fed each
				    other: canvas resize -> container content height changes ->
				    ResizeObserver fires -> canvas resize. That loop thrashed layout
				    and pushed the chart past the panel edge. */}
				<div ref={plotHostRef} className="absolute inset-0" />
				{!isRunning && visibleTraces.length === 0 && (
					<div className="absolute inset-0 flex items-center justify-center text-muted-foreground bg-background/50 z-10">
						Start audio stream and generator to measure transfer function
					</div>
				)}
			</div>
			<div className="mt-2 flex-shrink-0">
				<CursorReadout cursorA={cursorA} cursorB={cursorB} />
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
				{eqActive && predictedData && (
					<span className="flex items-center gap-1">
						<span
							className="w-4 h-0.5 bg-[oklch(0.55_0.2_30)] border-dashed"
							style={{ borderTopWidth: 1 }}
						/>
						Predicted (EQ)
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
