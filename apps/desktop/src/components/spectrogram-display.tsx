import { useCallback, useEffect, useRef, useState } from "react";
import { magnitudeToRgb } from "../lib/colormap";
import { type SpectrogramBuffer, createSpectrogramBuffer } from "../lib/spectrogram-buffer";

interface SpectrogramDisplayProps {
	spectrumData: number[];
	sampleRate: number;
	fftSize: number;
	isRunning: boolean;
}

/** Map linear FFT bins to log frequency bins */
const mapToLogBins = (
	linearData: number[],
	sampleRate: number,
	fftSize: number,
	numBins: number,
	minFreq: number,
	maxFreq: number,
): number[] => {
	const binWidth = sampleRate / fftSize;
	const logMin = Math.log10(minFreq);
	const logMax = Math.log10(maxFreq);
	const logStep = (logMax - logMin) / (numBins - 1);

	const result: number[] = [];
	for (let i = 0; i < numBins; i++) {
		const freq = 10 ** (logMin + i * logStep);
		const bin = Math.round(freq / binWidth);
		if (bin >= 0 && bin < linearData.length) {
			result.push(linearData[bin] ?? -120);
		} else {
			result.push(-120);
		}
	}
	return result;
};

/** Default spectrogram configuration */
const DEFAULT_MIN_DB = -100;
const DEFAULT_MAX_DB = -20;
const MIN_FREQ = 20;
const MAX_FREQ = 20000;

export const SpectrogramDisplay = ({
	spectrumData,
	sampleRate,
	fftSize,
	isRunning,
}: SpectrogramDisplayProps) => {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const containerRef = useRef<HTMLDivElement>(null);
	// Ring buffer of log-frequency-binned columns, one bin row per canvas pixel row.
	// Recreated only on resize, alongside imageDataRef and spectrumScratchRef below.
	const bufferRef = useRef<SpectrogramBuffer | null>(null);
	// Reused scratch for the log-mapped spectrum, sized to the buffer's bin count,
	// so pushing a new column never allocates.
	const spectrumScratchRef = useRef<Float32Array | null>(null);
	const imageDataRef = useRef<ImageData | null>(null);

	const [minDb, setMinDb] = useState(DEFAULT_MIN_DB);
	const [maxDb, setMaxDb] = useState(DEFAULT_MAX_DB);
	const [isPaused, setIsPaused] = useState(false);
	const [theme, setTheme] = useState<"dark" | "light">(() =>
		document.documentElement.classList.contains("dark") ? "dark" : "light",
	);

	// Pre-compute color lookup table
	const colorLutRef = useRef<Uint8ClampedArray>(new Uint8ClampedArray(256 * 4));

	// Watch for theme changes
	useEffect(() => {
		const observer = new MutationObserver(() => {
			setTheme(document.documentElement.classList.contains("dark") ? "dark" : "light");
		});
		observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
		return () => observer.disconnect();
	}, []);

	// Build the color LUT once. The 256 entries span the normalized 0-1 range that
	// the draw loop maps real dB values into, independent of the current minDb/maxDb.
	useEffect(() => {
		const lut = colorLutRef.current;
		for (let i = 0; i < 256; i++) {
			const [r, g, b] = magnitudeToRgb(i, 0, 255);
			lut[i * 4 + 0] = r;
			lut[i * 4 + 1] = g;
			lut[i * 4 + 2] = b;
			lut[i * 4 + 3] = 255;
		}
	}, []);

	// Initialize canvas and resize handling
	useEffect(() => {
		if (!containerRef.current || !canvasRef.current) return;

		const canvas = canvasRef.current;
		const container = containerRef.current;

		const updateSize = () => {
			const rect = container.getBoundingClientRect();
			const width = Math.floor(rect.width);
			const height = Math.floor(Math.max(rect.height, 200));

			if (canvas.width !== width || canvas.height !== height) {
				canvas.width = width;
				canvas.height = height;
				imageDataRef.current = null; // Force recreation
				// One bin row per pixel row, one history column per pixel column.
				bufferRef.current = createSpectrogramBuffer(height, width);
				spectrumScratchRef.current = new Float32Array(height);
			}
		};

		updateSize();

		const resizeObserver = new ResizeObserver(updateSize);
		resizeObserver.observe(container);

		return () => resizeObserver.disconnect();
	}, []);

	// Clear history
	const clearHistory = useCallback(() => {
		const canvas = canvasRef.current;
		if (!canvas) return;

		bufferRef.current = createSpectrogramBuffer(canvas.height, canvas.width);

		const ctx = canvas.getContext("2d");
		if (ctx) {
			ctx.fillStyle = theme === "dark" ? "#0a0a0a" : "#fafafa";
			ctx.fillRect(0, 0, canvas.width, canvas.height);
		}
	}, [theme]);

	// Update spectrogram
	const updateSpectrogram = useCallback(() => {
		if (!canvasRef.current || !isRunning || isPaused || spectrumData.length === 0) return;

		const canvas = canvasRef.current;
		const ctx = canvas.getContext("2d");
		if (!ctx) return;

		const width = canvas.width;
		const height = canvas.height;
		if (width === 0 || height === 0) return;

		const buffer = bufferRef.current;
		const scratch = spectrumScratchRef.current;
		if (!buffer || !scratch) return;

		// Map to log frequency scale and push the new column into the ring buffer.
		// mapToLogBins allocates its result, but it runs once per incoming spectrum
		// (not per pixel); copying into the reused scratch keeps the buffer push itself
		// allocation-free.
		const logData = mapToLogBins(spectrumData, sampleRate, fftSize, height, MIN_FREQ, MAX_FREQ);
		scratch.set(logData);
		buffer.push(scratch);

		// Create or reuse ImageData
		if (
			!imageDataRef.current ||
			imageDataRef.current.width !== width ||
			imageDataRef.current.height !== height
		) {
			imageDataRef.current = ctx.createImageData(width, height);
		}

		const imageData = imageDataRef.current;
		const data = imageData.data;
		const lut = colorLutRef.current;
		const dbRange = maxDb - minDb;
		const bg = theme === "dark" ? 10 : 250;

		// Fill the image data: columns oldest (left) to newest (right), bins bottom
		// (low frequency) to top (high frequency). No allocation here — this is the
		// per-frame path, and it only writes into `data` and calls putImageData once.
		for (let x = 0; x < width; x++) {
			const age = width - 1 - x;
			const hasColumn = age < buffer.length;
			const column = buffer.column(age);

			for (let row = 0; row < height; row++) {
				const y = height - 1 - row;
				const pixelIndex = (y * width + x) * 4;

				if (hasColumn) {
					const db = column[row] ?? -120;
					// Normalize to 0-1 range
					const normalized = Math.max(0, Math.min(1, (db - minDb) / dbRange));
					// Map to color LUT index (0-255)
					const lutIndex = Math.floor(normalized * 255) * 4;

					data[pixelIndex + 0] = lut[lutIndex + 0] ?? 0;
					data[pixelIndex + 1] = lut[lutIndex + 1] ?? 0;
					data[pixelIndex + 2] = lut[lutIndex + 2] ?? 0;
					data[pixelIndex + 3] = 255;
				} else {
					// Background color for empty areas
					data[pixelIndex + 0] = bg;
					data[pixelIndex + 1] = bg;
					data[pixelIndex + 2] = bg;
					data[pixelIndex + 3] = 255;
				}
			}
		}

		ctx.putImageData(imageData, 0, 0);

		// Draw frequency labels
		drawLabels(ctx, width, height);
	}, [spectrumData, sampleRate, fftSize, isRunning, isPaused, minDb, maxDb, theme]);

	// Draw frequency labels on canvas
	const drawLabels = (ctx: CanvasRenderingContext2D, width: number, height: number) => {
		const isDark = theme === "dark";
		ctx.font = "10px system-ui, sans-serif";
		ctx.fillStyle = isDark ? "rgba(255,255,255,0.7)" : "rgba(0,0,0,0.7)";
		ctx.strokeStyle = isDark ? "rgba(255,255,255,0.2)" : "rgba(0,0,0,0.2)";
		ctx.lineWidth = 1;

		// Frequency labels on right edge
		const freqLabels = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];
		const logMin = Math.log10(MIN_FREQ);
		const logMax = Math.log10(MAX_FREQ);

		for (const freq of freqLabels) {
			const logFreq = Math.log10(freq);
			const normalized = (logFreq - logMin) / (logMax - logMin);
			const y = height - normalized * height;

			// Draw horizontal guide line
			ctx.beginPath();
			ctx.moveTo(0, y);
			ctx.lineTo(width, y);
			ctx.stroke();

			// Draw label
			const label = freq >= 1000 ? `${freq / 1000}k` : String(freq);
			ctx.fillText(label, width - 28, y + 3);
		}
	};

	useEffect(() => {
		updateSpectrogram();
	}, [updateSpectrogram]);

	return (
		<div className="flex flex-col h-full min-h-0">
			<div className="flex items-center justify-between mb-2 flex-shrink-0">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Spectrogram
				</h2>
				<div className="flex items-center gap-2">
					<div className="flex items-center gap-1 text-xs text-muted-foreground">
						<span>Range:</span>
						<input
							type="number"
							value={minDb}
							onChange={(e) => setMinDb(Number(e.target.value))}
							className="w-14 px-1 py-0.5 text-xs rounded border border-border bg-background text-foreground"
							step={10}
						/>
						<span>to</span>
						<input
							type="number"
							value={maxDb}
							onChange={(e) => setMaxDb(Number(e.target.value))}
							className="w-14 px-1 py-0.5 text-xs rounded border border-border bg-background text-foreground"
							step={10}
						/>
						<span>dB</span>
					</div>
					<button
						type="button"
						onClick={() => setIsPaused(!isPaused)}
						className={`px-2 py-1 text-xs rounded border transition-colors ${
							isPaused
								? "border-yellow-500 bg-yellow-500/10 text-yellow-500"
								: "border-border hover:bg-accent"
						}`}
					>
						{isPaused ? "Resume" : "Pause"}
					</button>
					<button
						type="button"
						onClick={clearHistory}
						className="px-2 py-1 text-xs rounded border border-border hover:bg-accent"
					>
						Clear
					</button>
				</div>
			</div>
			<div ref={containerRef} data-testid="spectrogram" className="flex-1 min-h-[200px] relative">
				<canvas
					ref={canvasRef}
					data-testid="spectrogram-canvas"
					className="absolute inset-0 w-full h-full"
					style={{ imageRendering: "pixelated" }}
				/>
				{!isRunning && (
					<div className="absolute inset-0 flex items-center justify-center text-muted-foreground bg-background/50 z-10">
						Start audio stream to see spectrogram
					</div>
				)}
			</div>
			<div className="flex items-center gap-4 mt-2 text-xs text-muted-foreground flex-shrink-0">
				<span>Time →</span>
				<div className="flex-1" />
				<span className="flex items-center gap-2">
					<span
						className="w-24 h-3 rounded"
						style={{
							background: `linear-gradient(to right, ${Array.from({ length: 8 }, (_, i) => {
								const [r, g, b] = magnitudeToRgb(i, 0, 7);
								return `rgb(${r},${g},${b})`;
							}).join(", ")})`,
						}}
					/>
					<span>
						{minDb}dB → {maxDb}dB
					</span>
				</span>
			</div>
		</div>
	);
};
