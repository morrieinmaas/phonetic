import { useCallback, useEffect, useRef } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";
import type { TransferFunctionData } from "../services/audio-api";

interface TransferFunctionDisplayProps {
	data: TransferFunctionData | null;
	isRunning: boolean;
	onResetAveraging: () => void;
}

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

export const TransferFunctionDisplay = ({
	data,
	isRunning,
	onResetAveraging,
}: TransferFunctionDisplayProps) => {
	const containerRef = useRef<HTMLDivElement>(null);
	const chartRef = useRef<uPlot | null>(null);
	const logFreqsRef = useRef<number[]>([]);

	// Initialize log frequencies
	useEffect(() => {
		logFreqsRef.current = generateLogFrequencies(20, 20000, 512);
	}, []);

	// Create chart
	useEffect(() => {
		if (!containerRef.current) return;

		const container = containerRef.current;
		const width = container.clientWidth || 600;
		const height = Math.max(container.clientHeight, 250);

		const opts: uPlot.Options = {
			width,
			height,
			padding: [10, 20, 0, 0], // top, right, bottom, left
			cursor: {
				show: true,
				x: true,
				y: true,
			},
			scales: {
				x: {
					time: false,
					distr: 2, // Logarithmic scale
					log: 10,
					min: 20,
					max: 20000,
				},
				magnitude: {
					auto: false, // Force fixed range - never auto-scale
					range: [-40, 20],
				},
				phase: {
					auto: false, // Force fixed range - never auto-scale
					range: [-180, 180],
				},
			},
			axes: [
				{
					label: "Frequency (Hz)",
					labelSize: 20,
					labelFont: "11px system-ui, sans-serif",
					font: "10px system-ui, sans-serif",
					stroke: "var(--color-muted-foreground)",
					grid: { stroke: "var(--color-border)", width: 1 },
					ticks: { stroke: "var(--color-border)", width: 1, size: 5 },
					size: 40,
					gap: 5,
					splits: [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000],
					values: (_, ticks) =>
						ticks.map((v) => {
							if (v >= 10000) return `${(v / 1000).toFixed(0)}k`;
							if (v >= 1000) return `${(v / 1000).toFixed(0)}k`;
							return String(Math.round(v));
						}),
				},
				{
					label: "Magnitude (dB)",
					labelSize: 15,
					labelFont: "11px system-ui, sans-serif",
					font: "10px system-ui, sans-serif",
					stroke: "var(--color-muted-foreground)",
					grid: { stroke: "var(--color-border)", width: 1 },
					ticks: { stroke: "var(--color-border)", width: 1, size: 5 },
					size: 50,
					gap: 5,
					scale: "magnitude",
					splits: [-40, -30, -20, -10, 0, 10, 20],
				},
				{
					side: 1,
					label: "Phase (°)",
					labelSize: 15,
					labelFont: "11px system-ui, sans-serif",
					font: "10px system-ui, sans-serif",
					stroke: "var(--color-muted-foreground)",
					grid: { show: false },
					ticks: { stroke: "var(--color-border)", width: 1, size: 5 },
					size: 50,
					gap: 5,
					scale: "phase",
					splits: [-180, -90, 0, 90, 180],
				},
			],
			series: [
				{},
				{
					label: "Magnitude",
					stroke: "oklch(0.7 0.15 280)", // Primary/violet
					width: 2,
					scale: "magnitude",
				},
				{
					label: "Phase",
					stroke: "oklch(0.7 0.15 200)", // Cyan
					width: 1,
					scale: "phase",
					dash: [4, 2],
				},
			],
		};

		// Initial empty data
		const data: uPlot.AlignedData = [
			logFreqsRef.current,
			new Array(512).fill(0), // Magnitude
			new Array(512).fill(0), // Phase
		];

		chartRef.current = new uPlot(opts, data, container);

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
	}, []);

	// Update chart data
	const updateChart = useCallback(() => {
		if (!chartRef.current || !data || !isRunning) return;

		const logMagnitude = mapToLogScale(data.magnitude_db, data.frequencies, logFreqsRef.current);
		const logPhase = mapToLogScale(data.phase_deg, data.frequencies, logFreqsRef.current);

		chartRef.current.setData([logFreqsRef.current, logMagnitude, logPhase]);
	}, [data, isRunning]);

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
			<div className="flex items-center justify-between mb-2 flex-shrink-0">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Transfer Function
				</h2>
				<div className="flex items-center gap-4">
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
				{!isRunning && (
					<div className="absolute inset-0 flex items-center justify-center text-muted-foreground bg-background/50 z-10">
						Start audio stream and generator to measure transfer function
					</div>
				)}
			</div>
			<div className="flex gap-4 mt-2 text-xs text-muted-foreground flex-shrink-0">
				<span className="flex items-center gap-1">
					<span className="w-4 h-0.5 bg-[oklch(0.7_0.15_280)]" />
					Magnitude
				</span>
				<span className="flex items-center gap-1">
					<span
						className="w-4 h-0.5 bg-[oklch(0.7_0.15_200)] border-dashed"
						style={{ borderTopWidth: 1 }}
					/>
					Phase
				</span>
			</div>
		</div>
	);
};
