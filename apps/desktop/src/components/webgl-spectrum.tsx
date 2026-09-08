import { useCallback, useEffect, useRef, useState } from "react";
import { applyPan, applyZoom, pixelToLogFrequency } from "../lib/plot-scales";

interface WebGLSpectrumProps {
	spectrumData: number[];
	sampleRate: number;
	fftSize: number;
	isRunning: boolean;
	showWaterfall?: boolean;
}

// Theme colors for WebGL rendering
const THEME_COLORS = {
	dark: {
		background: [0.05, 0.05, 0.08] as const,
		labelFill: "#e5e5e5",
		gridStroke: "rgba(255,255,255,0.1)",
		containerBg: "bg-[#15151d]",
		peakHold: "#f59e0b",
	},
	light: {
		background: [0.97, 0.97, 0.98] as const,
		labelFill: "#171717",
		gridStroke: "rgba(0,0,0,0.15)",
		containerBg: "bg-[#eeeef3]",
		peakHold: "#d97706",
	},
};

// Vertex shader - positions the spectrum bars
const VERTEX_SHADER = `
  attribute vec2 a_position;
  attribute float a_magnitude;
  uniform vec2 u_resolution;
  uniform float u_minDb;
  uniform float u_maxDb;
  varying float v_magnitude;

  void main() {
    // Convert from pixel coords to clip space (-1 to 1)
    vec2 clipSpace = (a_position / u_resolution) * 2.0 - 1.0;
    // Flip Y axis
    clipSpace.y = -clipSpace.y;
    gl_Position = vec4(clipSpace, 0, 1);
    v_magnitude = (a_magnitude - u_minDb) / (u_maxDb - u_minDb);
  }
`;

// Fragment shader - colors the spectrum
const FRAGMENT_SHADER = `
  precision mediump float;
  varying float v_magnitude;
  uniform vec3 u_colorLow;
  uniform vec3 u_colorMid;
  uniform vec3 u_colorHigh;

  void main() {
    vec3 color;
    if (v_magnitude < 0.5) {
      color = mix(u_colorLow, u_colorMid, v_magnitude * 2.0);
    } else {
      color = mix(u_colorMid, u_colorHigh, (v_magnitude - 0.5) * 2.0);
    }
    gl_FragColor = vec4(color, 1.0);
  }
`;

// Generate logarithmic frequency points (20Hz to 20kHz)
const generateLogFrequencies = (minFreq: number, maxFreq: number, numPoints: number): number[] => {
	const logMin = Math.log10(minFreq);
	const logMax = Math.log10(maxFreq);
	const step = (logMax - logMin) / (numPoints - 1);
	return Array.from({ length: numPoints }, (_, i) => 10 ** (logMin + i * step));
};

// Map FFT bin to pixel X position (logarithmic scale)
const freqToX = (freq: number, width: number, minFreq = 20, maxFreq = 20000): number => {
	const logMin = Math.log10(minFreq);
	const logMax = Math.log10(maxFreq);
	const logFreq = Math.log10(Math.max(freq, minFreq));
	return ((logFreq - logMin) / (logMax - logMin)) * width;
};

// Map dB value to pixel Y position
const dbToY = (db: number, height: number, minDb = -120, maxDb = 0): number => {
	const normalized = (db - minDb) / (maxDb - minDb);
	return height * (1 - Math.max(0, Math.min(1, normalized)));
};

/** Full frequency range of the plot, and the limit that zoom/pan gestures cannot cross. */
const FREQ_LIMITS: [number, number] = [20, 20000];

// Create and compile a shader
const createShader = (
	gl: WebGLRenderingContext,
	type: number,
	source: string,
): WebGLShader | null => {
	const shader = gl.createShader(type);
	if (!shader) return null;

	gl.shaderSource(shader, source);
	gl.compileShader(shader);

	if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
		console.error("Shader compile error:", gl.getShaderInfoLog(shader));
		gl.deleteShader(shader);
		return null;
	}

	return shader;
};

// Create shader program
const createProgram = (
	gl: WebGLRenderingContext,
	vertexShader: WebGLShader,
	fragmentShader: WebGLShader,
): WebGLProgram | null => {
	const program = gl.createProgram();
	if (!program) return null;

	gl.attachShader(program, vertexShader);
	gl.attachShader(program, fragmentShader);
	gl.linkProgram(program);

	if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
		console.error("Program link error:", gl.getProgramInfoLog(program));
		gl.deleteProgram(program);
		return null;
	}

	return program;
};

export const WebGLSpectrum = ({
	spectrumData,
	sampleRate,
	fftSize,
	isRunning,
	showWaterfall = false,
}: WebGLSpectrumProps) => {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const peakCanvasRef = useRef<HTMLCanvasElement>(null);
	const plotContainerRef = useRef<HTMLDivElement>(null);
	const freqRangeRef = useRef<[number, number]>(FREQ_LIMITS);
	const glRef = useRef<WebGLRenderingContext | null>(null);
	const programRef = useRef<WebGLProgram | null>(null);
	const positionBufferRef = useRef<WebGLBuffer | null>(null);
	const waterfallTextureRef = useRef<WebGLTexture | null>(null);
	const waterfallDataRef = useRef<Float32Array | null>(null);
	const peakHoldRef = useRef<number[]>([]);
	const waterfallRowRef = useRef(0);
	void waterfallRowRef;

	const [peakHoldEnabled, setPeakHoldEnabled] = useState(true);
	const skipPeakUntilRef = useRef(0); // Timestamp until which to skip peak updates

	// Theme detection
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

	const themeColors = THEME_COLORS[theme];

	const MIN_DB = -120;
	const MAX_DB = 0;
	const WATERFALL_ROWS = 256;

	// Initialize peak hold array
	useEffect(() => {
		peakHoldRef.current = new Array(512).fill(-120);
	}, []);

	const clearPeakHold = () => {
		peakHoldRef.current = new Array(512).fill(-120);
		skipPeakUntilRef.current = Date.now() + 1500; // Skip peak updates for 1.5s so user can see the clear
	};

	// Initialize WebGL
	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas) return;

		const gl = canvas.getContext("webgl", {
			alpha: false,
			antialias: true,
			preserveDrawingBuffer: true,
		});

		if (!gl) {
			console.error("WebGL not supported");
			return;
		}

		glRef.current = gl;

		// Create shaders
		const vertexShader = createShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
		const fragmentShader = createShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);

		if (!vertexShader || !fragmentShader) return;

		// Create program
		const program = createProgram(gl, vertexShader, fragmentShader);
		if (!program) return;

		programRef.current = program;

		// Create buffers
		positionBufferRef.current = gl.createBuffer();

		// Initialize waterfall texture if enabled
		if (showWaterfall) {
			waterfallTextureRef.current = gl.createTexture();
			waterfallDataRef.current = new Float32Array(512 * WATERFALL_ROWS);
		}

		// Set initial clear color
		const [r, g, b] = THEME_COLORS.dark.background;
		gl.clearColor(r, g, b, 1.0);

		// Enable blending for smooth lines
		gl.enable(gl.BLEND);
		gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);

		return () => {
			gl.deleteProgram(program);
			gl.deleteShader(vertexShader);
			gl.deleteShader(fragmentShader);
			if (positionBufferRef.current) gl.deleteBuffer(positionBufferRef.current);
			if (waterfallTextureRef.current) gl.deleteTexture(waterfallTextureRef.current);
		};
	}, [showWaterfall]);

	// Render spectrum
	const renderSpectrum = useCallback(() => {
		const canvas = canvasRef.current;
		const peakCanvas = peakCanvasRef.current;
		const gl = glRef.current;
		const program = programRef.current;

		if (!canvas || !gl || !program) return;

		// If no spectrum data, just clear the peak canvas and return
		if (spectrumData.length === 0) {
			if (peakCanvas) {
				const ctx = peakCanvas.getContext("2d");
				if (ctx) ctx.clearRect(0, 0, peakCanvas.width, peakCanvas.height);
			}
			return;
		}

		const width = canvas.width;
		const height = canvas.height;

		// Update clear color for current theme
		const [r, g, b] = themeColors.background;
		gl.clearColor(r, g, b, 1.0);

		gl.viewport(0, 0, width, height);
		gl.clear(gl.COLOR_BUFFER_BIT);
		gl.useProgram(program);

		// Set uniforms
		const resolutionLoc = gl.getUniformLocation(program, "u_resolution");
		const minDbLoc = gl.getUniformLocation(program, "u_minDb");
		const maxDbLoc = gl.getUniformLocation(program, "u_maxDb");
		const colorLowLoc = gl.getUniformLocation(program, "u_colorLow");
		const colorMidLoc = gl.getUniformLocation(program, "u_colorMid");
		const colorHighLoc = gl.getUniformLocation(program, "u_colorHigh");

		gl.uniform2f(resolutionLoc, width, height);
		gl.uniform1f(minDbLoc, MIN_DB);
		gl.uniform1f(maxDbLoc, MAX_DB);

		// Spectrum colors (purple gradient)
		gl.uniform3f(colorLowLoc, 0.2, 0.1, 0.3);
		gl.uniform3f(colorMidLoc, 0.5, 0.2, 0.8);
		gl.uniform3f(colorHighLoc, 0.9, 0.4, 1.0);

		// Build vertex data for spectrum bars
		const binWidth = sampleRate / fftSize;
		const vertices: number[] = [];
		const magnitudes: number[] = [];
		const logDbValues: number[] = [];

		// Log frequency points for smooth curve
		const numPoints = 512;
		const logFreqs = generateLogFrequencies(20, 20000, numPoints);

		for (let i = 0; i < numPoints - 1; i++) {
			const freq = logFreqs[i];
			const nextFreq = logFreqs[i + 1];

			if (freq === undefined || nextFreq === undefined) continue;

			// Find corresponding FFT bin
			const bin = Math.round(freq / binWidth);
			const nextBin = Math.round(nextFreq / binWidth);

			if (bin < 0 || bin >= spectrumData.length) continue;

			// Average bins for smoother display
			let sum = 0;
			let count = 0;
			for (let b = bin; b <= Math.min(nextBin, spectrumData.length - 1); b++) {
				const value = spectrumData[b];
				if (value !== undefined) {
					sum += value;
					count++;
				}
			}
			const db = count > 0 ? sum / count : MIN_DB;
			logDbValues.push(db);

			const x1 = freqToX(freq, width, freqRangeRef.current[0], freqRangeRef.current[1]);
			const x2 = freqToX(nextFreq, width, freqRangeRef.current[0], freqRangeRef.current[1]);
			const y1 = height;
			const y2 = dbToY(db, height, MIN_DB, MAX_DB);

			// Create a filled rectangle (two triangles)
			vertices.push(x1, y1, x1, y2, x2, y2);
			vertices.push(x1, y1, x2, y2, x2, y1);

			const normalizedMag = db;
			for (let j = 0; j < 6; j++) {
				magnitudes.push(normalizedMag);
			}
		}

		// Update peak hold (skip if within cooldown after clear)
		if (peakHoldEnabled && Date.now() > skipPeakUntilRef.current) {
			for (let i = 0; i < logDbValues.length; i++) {
				const currentPeak = peakHoldRef.current[i] ?? -120;
				const currentValue = logDbValues[i] ?? -120;
				if (currentValue > currentPeak) {
					peakHoldRef.current[i] = currentValue;
				} else {
					peakHoldRef.current[i] = Math.max(currentPeak - 0.5, currentValue);
				}
			}
		}

		// Upload vertex data
		const positionLoc = gl.getAttribLocation(program, "a_position");
		const magnitudeLoc = gl.getAttribLocation(program, "a_magnitude");

		gl.bindBuffer(gl.ARRAY_BUFFER, positionBufferRef.current);
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.DYNAMIC_DRAW);
		gl.enableVertexAttribArray(positionLoc);
		gl.vertexAttribPointer(positionLoc, 2, gl.FLOAT, false, 0, 0);

		const magBuffer = gl.createBuffer();
		gl.bindBuffer(gl.ARRAY_BUFFER, magBuffer);
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(magnitudes), gl.DYNAMIC_DRAW);
		gl.enableVertexAttribArray(magnitudeLoc);
		gl.vertexAttribPointer(magnitudeLoc, 1, gl.FLOAT, false, 0, 0);

		gl.drawArrays(gl.TRIANGLES, 0, vertices.length / 2);
		gl.deleteBuffer(magBuffer);

		// Draw peak hold line on 2D canvas overlay
		if (peakCanvas && peakHoldEnabled) {
			const ctx = peakCanvas.getContext("2d");
			if (ctx) {
				ctx.clearRect(0, 0, peakCanvas.width, peakCanvas.height);
				ctx.strokeStyle = themeColors.peakHold;
				ctx.lineWidth = 2 * window.devicePixelRatio;
				ctx.setLineDash([4 * window.devicePixelRatio, 2 * window.devicePixelRatio]);
				ctx.beginPath();

				for (let i = 0; i < numPoints - 1; i++) {
					const freq = logFreqs[i];
					if (freq === undefined) continue;

					const x = freqToX(freq, width, freqRangeRef.current[0], freqRangeRef.current[1]);
					const y = dbToY(peakHoldRef.current[i] ?? -120, height, MIN_DB, MAX_DB);

					if (i === 0) {
						ctx.moveTo(x, y);
					} else {
						ctx.lineTo(x, y);
					}
				}
				ctx.stroke();
			}
		} else if (peakCanvas) {
			const ctx = peakCanvas.getContext("2d");
			if (ctx) {
				ctx.clearRect(0, 0, peakCanvas.width, peakCanvas.height);
			}
		}
	}, [spectrumData, sampleRate, fftSize, themeColors, peakHoldEnabled]);

	// Handle resize
	useEffect(() => {
		const canvas = canvasRef.current;
		const peakCanvas = peakCanvasRef.current;
		if (!canvas) return;

		const resizeObserver = new ResizeObserver((entries) => {
			for (const entry of entries) {
				const { width, height } = entry.contentRect;
				canvas.width = width * window.devicePixelRatio;
				canvas.height = height * window.devicePixelRatio;
				canvas.style.width = `${width}px`;
				canvas.style.height = `${height}px`;
				if (peakCanvas) {
					peakCanvas.width = width * window.devicePixelRatio;
					peakCanvas.height = height * window.devicePixelRatio;
					peakCanvas.style.width = `${width}px`;
					peakCanvas.style.height = `${height}px`;
				}
			}
		});

		resizeObserver.observe(canvas);
		return () => resizeObserver.disconnect();
	}, []);

	// Render on data change
	useEffect(() => {
		if (isRunning && spectrumData.length > 0) {
			requestAnimationFrame(renderSpectrum);
		}
	}, [spectrumData, isRunning, renderSpectrum]);

	// Trackpad gestures: ctrl+wheel pinch zooms about the pointer, plain wheel pans.
	useEffect(() => {
		const container = plotContainerRef.current;
		if (!container) return;

		const handleWheel = (event: WheelEvent) => {
			event.preventDefault();

			const rect = container.getBoundingClientRect();
			const width = rect.width || 1;
			const pointerX = event.clientX - rect.left;
			const [lo, hi] = freqRangeRef.current;
			const anchor = pixelToLogFrequency(pointerX, width, lo, hi);

			freqRangeRef.current = event.ctrlKey
				? applyZoom(freqRangeRef.current, Math.exp(event.deltaY * 0.01), anchor, FREQ_LIMITS)
				: applyPan(freqRangeRef.current, (event.deltaX || event.deltaY) / width, FREQ_LIMITS);

			requestAnimationFrame(renderSpectrum);
		};

		container.addEventListener("wheel", handleWheel, { passive: false });
		return () => container.removeEventListener("wheel", handleWheel);
	}, [renderSpectrum]);

	// Double-click resets the plot back to the full frequency range.
	const handleResetZoom = useCallback(() => {
		freqRangeRef.current = FREQ_LIMITS;
		requestAnimationFrame(renderSpectrum);
	}, [renderSpectrum]);

	return (
		<div className="flex flex-col h-full min-h-0">
			<div className="flex items-center justify-between mb-2 flex-shrink-0">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Spectrum Analyzer (WebGL)
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
					<span className="text-xs text-muted-foreground px-2 py-0.5 rounded bg-green-500/10 text-green-500">
						GPU
					</span>
				</div>
			</div>
			<div
				ref={plotContainerRef}
				className={`flex-1 min-h-[280px] relative rounded-lg overflow-hidden ${themeColors.containerBg}`}
				onDoubleClick={handleResetZoom}
			>
				<canvas ref={canvasRef} className="absolute inset-0 w-full h-full" />
				<canvas
					ref={peakCanvasRef}
					className="absolute inset-0 w-full h-full pointer-events-none"
				/>
				{/* Grid overlay using SVG */}
				<svg
					className="absolute inset-0 w-full h-full pointer-events-none"
					preserveAspectRatio="none"
					aria-labelledby="spectrum-grid-title"
				>
					<title id="spectrum-grid-title">Spectrum analyzer frequency and amplitude grid</title>
					<defs>
						<pattern id="grid" width="100" height="50" patternUnits="userSpaceOnUse">
							<line
								x1="0"
								y1="50"
								x2="100"
								y2="50"
								stroke={themeColors.gridStroke}
								strokeWidth="0.5"
							/>
						</pattern>
					</defs>
					{/* Frequency labels */}
					<text x="5%" y="96%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						20
					</text>
					<text x="20%" y="96%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						100
					</text>
					<text x="40%" y="96%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						1k
					</text>
					<text x="60%" y="96%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						5k
					</text>
					<text x="80%" y="96%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						10k
					</text>
					<text x="95%" y="96%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						20k
					</text>
					{/* dB labels */}
					<text x="2%" y="8%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						0dB
					</text>
					<text x="2%" y="30%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						-30
					</text>
					<text x="2%" y="55%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						-60
					</text>
					<text x="2%" y="80%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						-90
					</text>
					<text x="2%" y="95%" fill={themeColors.labelFill} fontSize="12" fontWeight="bold">
						-120
					</text>
				</svg>
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
							className="w-4 h-0.5 bg-amber-500"
							style={{ borderTopWidth: 1, borderStyle: "dashed" }}
						/>
						Peak Hold
					</span>
				)}
			</div>
		</div>
	);
};
