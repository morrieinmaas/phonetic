import { useCallback, useEffect, useRef } from "react";
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

	// Initialize log frequencies
	useEffect(() => {
		logFreqsRef.current = generateLogFrequencies(20, 20000, 512);
	}, []);

	// Create chart
	useEffect(() => {
		if (!containerRef.current) return;

		const container = containerRef.current;
		const width = container.clientWidth || 600;
		const height = Math.max(container.clientHeight, 280);

		const opts: uPlot.Options = {
			width,
			height,
			padding: [10, 20, 0, 0], // top, right, bottom, left - give room for labels
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
				y: {
					auto: false, // Force fixed range - never auto-scale
					range: [-120, 0],
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
					size: 40, // Height of X-axis area
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
					label: "dB",
					labelSize: 15,
					labelFont: "11px system-ui, sans-serif",
					font: "10px system-ui, sans-serif",
					stroke: "var(--color-muted-foreground)",
					grid: { stroke: "var(--color-border)", width: 1 },
					ticks: { stroke: "var(--color-border)", width: 1, size: 5 },
					size: 50, // Width of Y-axis area
					gap: 5,
					splits: [-120, -100, -80, -60, -40, -20, 0],
				},
			],
			series: [
				{},
				{
					label: "Magnitude",
					stroke: "oklch(0.7 0.15 280)", // Primary color
					width: 2,
					fill: "oklch(0.7 0.15 280 / 0.1)",
				},
			],
		};

		// Initial empty data
		const data: uPlot.AlignedData = [logFreqsRef.current, new Array(512).fill(-120)];

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
	}, []);

	// Update chart data
	const updateChart = useCallback(() => {
		if (!chartRef.current || spectrumData.length === 0 || !isRunning) return;

		const logData = mapToLogScale(spectrumData, sampleRate, fftSize, logFreqsRef.current);
		chartRef.current.setData([logFreqsRef.current, logData]);
	}, [spectrumData, sampleRate, fftSize, isRunning]);

	useEffect(() => {
		updateChart();
	}, [updateChart]);

	return (
		<div className="flex flex-col h-full min-h-0">
			<h2 className="mb-2 text-sm font-semibold uppercase tracking-wider text-muted-foreground flex-shrink-0">
				Spectrum Analyzer
			</h2>
			<div ref={containerRef} className="flex-1 min-h-[280px] relative">
				{!isRunning && (
					<div className="absolute inset-0 flex items-center justify-center text-muted-foreground bg-background/50 z-10">
						Start audio stream to see spectrum
					</div>
				)}
			</div>
		</div>
	);
};
