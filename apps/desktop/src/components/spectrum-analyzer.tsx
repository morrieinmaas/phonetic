import { useCallback, useEffect, useRef, useState } from "react";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

interface SpectrumAnalyzerProps {
	spectrumData: number[];
	sampleRate: number;
	fftSize: number;
	isRunning: boolean;
}

// Generate logarithmic frequency labels
const generateLogFrequencies = (minFreq: number, maxFreq: number, numPoints: number): number[] => {
	const logMin = Math.log10(minFreq);
	const logMax = Math.log10(maxFreq);
	const step = (logMax - logMin) / (numPoints - 1);

	return Array.from({ length: numPoints }, (_, i) => 10 ** (logMin + i * step));
};

// Map linear FFT bins to log frequency scale
const mapToLogScale = (
	linearData: number[],
	sampleRate: number,
	fftSize: number,
	logFreqs: number[],
): number[] => {
	const binWidth = sampleRate / fftSize;

	return logFreqs.map((freq) => {
		const bin = Math.round(freq / binWidth);
		if (bin < 0 || bin >= linearData.length) {
			return -120;
		}
		return linearData[bin] ?? -120;
	});
};

export const SpectrumAnalyzer = ({
	spectrumData,
	sampleRate,
	fftSize,
	isRunning,
}: SpectrumAnalyzerProps) => {
	const containerRef = useRef<HTMLDivElement>(null);
	const chartRef = useRef<uPlot | null>(null);
	const logFreqsRef = useRef<number[]>([]);
	const peakHoldRef = useRef<number[]>([]);
	const [peakHoldEnabled, setPeakHoldEnabled] = useState(true);
	const skipPeakUntilRef = useRef(0); // Timestamp until which to skip peak updates
	const [theme, setTheme] = useState<"dark" | "light">(() =>
		document.documentElement.classList.contains("dark") ? "dark" : "light",
	);

	// Watch for theme changes
	useEffect(() => {
		const observer = new MutationObserver(() => {
			setTheme(document.documentElement.classList.contains("dark") ? "dark" : "light");
		});
		observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
		return () => observer.disconnect();
	}, []);

	// Initialize log frequencies and peak hold
	useEffect(() => {
		logFreqsRef.current = generateLogFrequencies(20, 20000, 512);
		peakHoldRef.current = new Array(512).fill(-120);
	}, []);

	// Clear peak hold - resets values and pauses accumulation briefly
	const clearPeakHold = () => {
		peakHoldRef.current = new Array(512).fill(-120);
		skipPeakUntilRef.current = Date.now() + 750; // Skip peak updates for 1.5s so user can see the clear
	};

	// Create chart (recreate when theme changes)
	useEffect(() => {
		if (!containerRef.current) return;

		const container = containerRef.current;
		const width = container.clientWidth || 600;
		const height = Math.max(container.clientHeight, 280);

		// Theme-specific colors with high contrast
		const isDark = theme === "dark";
		const labelColor = isDark ? "#f5f5f5" : "#171717";
		const gridColor = isDark ? "rgba(255,255,255,0.2)" : "rgba(0,0,0,0.2)";
		const tickColor = isDark ? "rgba(255,255,255,0.4)" : "rgba(0,0,0,0.4)";

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
				y: {
					auto: false,
					range: [-120, 0],
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
					label: "dB",
					labelSize: 20,
					labelFont: "bold 13px system-ui, sans-serif",
					font: "bold 12px system-ui, sans-serif",
					stroke: labelColor,
					grid: { stroke: gridColor, width: 1 },
					ticks: { stroke: tickColor, width: 1, size: 6 },
					size: 55,
					gap: 8,
					splits: [-120, -100, -80, -60, -40, -20, 0],
				},
			],
			series: [
				{},
				{
					label: "Magnitude",
					stroke: isDark ? "oklch(0.75 0.18 280)" : "oklch(0.55 0.2 280)",
					width: 2,
					fill: isDark ? "oklch(0.75 0.18 280 / 0.15)" : "oklch(0.55 0.2 280 / 0.15)",
				},
				{
					label: "Peak Hold",
					stroke: isDark ? "oklch(0.85 0.2 50)" : "oklch(0.6 0.25 50)",
					width: 1.5,
					dash: [2, 2],
				},
			],
		};

		// Initial empty data
		const data: uPlot.AlignedData = [
			logFreqsRef.current,
			new Array(512).fill(-120),
			new Array(512).fill(-120),
		];

		chartRef.current = new uPlot(opts, data, container);

		// Handle resize
		const resizeObserver = new ResizeObserver((entries) => {
			for (const entry of entries) {
				if (chartRef.current && entry.contentRect.width > 0) {
					chartRef.current.setSize({
						width: entry.contentRect.width,
						height: Math.max(entry.contentRect.height, 280),
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
	}, [theme]);

	// Update chart data
	const updateChart = useCallback(() => {
		if (!chartRef.current) return;

		// Use current spectrum data or empty if not running/no data
		const hasData = spectrumData.length > 0 && isRunning;
		const logData = hasData
			? mapToLogScale(spectrumData, sampleRate, fftSize, logFreqsRef.current)
			: new Array(512).fill(-120);

		// Update peak hold only when streaming (skip if within cooldown after clear)
		if (peakHoldEnabled && hasData && Date.now() > skipPeakUntilRef.current) {
			for (let i = 0; i < logData.length; i++) {
				const currentPeak = peakHoldRef.current[i] ?? -120;
				const currentValue = logData[i] ?? -120;
				// Update peak if current value is higher
				if (currentValue > currentPeak) {
					peakHoldRef.current[i] = currentValue;
				} else {
					// Slow decay (0.5 dB per frame)
					peakHoldRef.current[i] = Math.max(currentPeak - 0.5, currentValue);
				}
			}
		}

		const peakData = peakHoldEnabled ? [...peakHoldRef.current] : new Array(512).fill(-120);
		chartRef.current.setData([logFreqsRef.current, logData, peakData]);
	}, [spectrumData, sampleRate, fftSize, isRunning, peakHoldEnabled]);

	// Update chart when data changes
	useEffect(() => {
		updateChart();
	}, [updateChart]);

	return (
		<div className="flex flex-col h-full min-h-0">
			<div className="flex items-center justify-between mb-2 flex-shrink-0">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Spectrum Analyzer
				</h2>
				<div className="flex items-center gap-2">
					<button
						type="button"
						onClick={() => {
							setPeakHoldEnabled(!peakHoldEnabled);
							if (peakHoldEnabled) clearPeakHold();
						}}
						className={`px-2 py-1 text-xs rounded border transition-colors ${
							peakHoldEnabled
								? "border-primary bg-primary/10 text-primary"
								: "border-border hover:bg-accent"
						}`}
					>
						Peak Hold
					</button>
					{peakHoldEnabled && (
						<button
							type="button"
							onClick={clearPeakHold}
							className="px-2 py-1 text-xs rounded border border-border hover:bg-accent"
						>
							Clear
						</button>
					)}
				</div>
			</div>
			<div ref={containerRef} className="flex-1 min-h-[280px] relative">
				{!isRunning && (
					<div className="absolute inset-0 flex items-center justify-center text-muted-foreground bg-background/50 z-10">
						Start audio stream to see spectrum
					</div>
				)}
			</div>
			<div className="flex gap-4 mt-2 text-xs text-muted-foreground flex-shrink-0">
				<span className="flex items-center gap-1">
					<span className="w-4 h-0.5 bg-[oklch(0.7_0.15_280)]" />
					Magnitude
				</span>
				{peakHoldEnabled && (
					<span className="flex items-center gap-1">
						<span
							className="w-4 h-0.5 bg-[oklch(0.7_0.2_50)]"
							style={{ borderTopWidth: 1, borderStyle: "dashed" }}
						/>
						Peak Hold
					</span>
				)}
			</div>
		</div>
	);
};
