import { useCallback, useEffect, useRef, useState } from "react";

interface SpectrogramDisplayProps {
	spectrumData: number[];
	sampleRate: number;
	fftSize: number;
	isRunning: boolean;
}

type ColorMap = "viridis" | "plasma" | "inferno" | "magma" | "turbo";

/** Color maps for spectrogram display (256 RGB values each) */
const COLOR_MAPS: Record<ColorMap, (t: number) => [number, number, number]> = {
	viridis: (t) => {
		// Viridis colormap approximation
		const r = Math.round(
			255 * (0.267 + 0.005 * t + 2.71 * t * t - 12.9 * t * t * t + 16.3 * t ** 4 - 6.5 * t ** 5),
		);
		const g = Math.round(
			255 * (0.004 + 1.42 * t - 1.54 * t * t + 1.15 * t * t * t - 0.18 * t ** 4),
		);
		const b = Math.round(
			255 * (0.329 + 1.44 * t - 2.88 * t * t + 2.13 * t * t * t - 0.57 * t ** 4),
		);
		return [
			Math.max(0, Math.min(255, r)),
			Math.max(0, Math.min(255, g)),
			Math.max(0, Math.min(255, b)),
		];
	},
	plasma: (t) => {
		const r = Math.round(255 * (0.05 + 2.8 * t - 2.2 * t * t));
		const g = Math.round(255 * (0.02 + 0.4 * t + 0.85 * t * t - 0.25 * t * t * t));
		const b = Math.round(255 * (0.53 + 0.65 * t - 2.2 * t * t + 1.1 * t * t * t));
		return [
			Math.max(0, Math.min(255, r)),
			Math.max(0, Math.min(255, g)),
			Math.max(0, Math.min(255, b)),
		];
	},
	inferno: (t) => {
		const r = Math.round(255 * (0.0 + 3.6 * t - 2.9 * t * t));
		const g = Math.round(255 * (0.0 + 0.5 * t + 1.1 * t * t - 0.6 * t * t * t));
		const b = Math.round(255 * (0.01 + 2.2 * t - 3.8 * t * t + 2.0 * t * t * t));
		return [
			Math.max(0, Math.min(255, r)),
			Math.max(0, Math.min(255, g)),
			Math.max(0, Math.min(255, b)),
		];
	},
	magma: (t) => {
		const r = Math.round(255 * (0.0 + 2.8 * t - 1.7 * t * t));
		const g = Math.round(255 * (0.0 + 0.3 * t + 1.0 * t * t - 0.3 * t * t * t));
		const b = Math.round(255 * (0.02 + 2.4 * t - 3.2 * t * t + 1.6 * t * t * t));
		return [
			Math.max(0, Math.min(255, r)),
			Math.max(0, Math.min(255, g)),
			Math.max(0, Math.min(255, b)),
		];
	},
	turbo: (t) => {
		// Turbo colormap - good perceptual uniformity
		const r = Math.round(
			255 * Math.max(0, Math.min(1, 0.13 + 4.3 * t - 13.4 * t * t + 16.5 * t ** 3 - 6.8 * t ** 4)),
		);
		const g = Math.round(
			255 * Math.max(0, Math.min(1, 0.09 + 2.9 * t - 5.5 * t * t + 5.1 * t ** 3 - 1.7 * t ** 4)),
		);
		const b = Math.round(
			255 * Math.max(0, Math.min(1, 0.11 + 3.2 * t - 9.2 * t * t + 9.8 * t ** 3 - 3.4 * t ** 4)),
		);
		return [r, g, b];
	},
};

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
	const historyRef = useRef<number[][]>([]);
	const imageDataRef = useRef<ImageData | null>(null);

	const [colorMap, setColorMap] = useState<ColorMap>("plasma");
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

	// Update color LUT when colormap changes
	useEffect(() => {
		const lut = colorLutRef.current;
		const mapFn = COLOR_MAPS[colorMap];
		for (let i = 0; i < 256; i++) {
			const t = i / 255;
			const [r, g, b] = mapFn(t);
			lut[i * 4 + 0] = r;
			lut[i * 4 + 1] = g;
			lut[i * 4 + 2] = b;
			lut[i * 4 + 3] = 255;
		}
	}, [colorMap]);

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
			}
		};

		updateSize();

		const resizeObserver = new ResizeObserver(updateSize);
		resizeObserver.observe(container);

		return () => resizeObserver.disconnect();
	}, []);

	// Clear history
	const clearHistory = useCallback(() => {
		historyRef.current = [];
		if (canvasRef.current) {
			const ctx = canvasRef.current.getContext("2d");
			if (ctx) {
				ctx.fillStyle = theme === "dark" ? "#0a0a0a" : "#fafafa";
				ctx.fillRect(0, 0, canvasRef.current.width, canvasRef.current.height);
			}
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

		// Map to log frequency scale
		const numBins = height;
		const logData = mapToLogBins(spectrumData, sampleRate, fftSize, numBins, MIN_FREQ, MAX_FREQ);

		// Add to history
		historyRef.current.push(logData);
		if (historyRef.current.length > width) {
			historyRef.current.shift();
		}

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
		const history = historyRef.current;
		const historyLen = history.length;
		const dbRange = maxDb - minDb;

		// Fill the image data
		for (let x = 0; x < width; x++) {
			const historyIndex = x - (width - historyLen);
			const column = historyIndex >= 0 && historyIndex < historyLen ? history[historyIndex] : null;

			for (let y = 0; y < height; y++) {
				// Flip Y axis so low frequencies are at bottom
				const freqIndex = height - 1 - y;
				const pixelIndex = (y * width + x) * 4;

				if (column && freqIndex < column.length) {
					const db = column[freqIndex] ?? -120;
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
					const bg = theme === "dark" ? 10 : 250;
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
					<select
						value={colorMap}
						onChange={(e) => setColorMap(e.target.value as ColorMap)}
						className="px-2 py-1 text-xs rounded border border-border bg-background text-foreground"
					>
						<option value="turbo">Turbo</option>
						<option value="viridis">Viridis</option>
						<option value="plasma">Plasma</option>
						<option value="inferno">Inferno</option>
						<option value="magma">Magma</option>
					</select>
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
			<div ref={containerRef} className="flex-1 min-h-[200px] relative">
				<canvas
					ref={canvasRef}
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
								const t = i / 7;
								const [r, g, b] = COLOR_MAPS[colorMap](t);
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
