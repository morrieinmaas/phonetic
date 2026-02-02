import { useCallback, useEffect, useRef } from "react";

interface WebGLSpectrumProps {
	spectrumData: number[];
	sampleRate: number;
	fftSize: number;
	isRunning: boolean;
	showWaterfall?: boolean;
}

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
	const glRef = useRef<WebGLRenderingContext | null>(null);
	const programRef = useRef<WebGLProgram | null>(null);
	const positionBufferRef = useRef<WebGLBuffer | null>(null);
	const waterfallTextureRef = useRef<WebGLTexture | null>(null);
	const waterfallDataRef = useRef<Float32Array | null>(null);
	// Used for waterfall scrolling (incremented but read when rendering waterfall)
	const waterfallRowRef = useRef(0);
	// Suppress unused warning - will be used when waterfall rendering is implemented
	void waterfallRowRef;

	const MIN_DB = -120;
	const MAX_DB = 0;
	const WATERFALL_ROWS = 256;

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

		// Set clear color (dark background)
		gl.clearColor(0.05, 0.05, 0.08, 1.0);

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
		const gl = glRef.current;
		const program = programRef.current;

		if (!canvas || !gl || !program || spectrumData.length === 0) return;

		const width = canvas.width;
		const height = canvas.height;

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
		gl.uniform3f(colorLowLoc, 0.2, 0.1, 0.3); // Dark purple
		gl.uniform3f(colorMidLoc, 0.5, 0.2, 0.8); // Medium purple
		gl.uniform3f(colorHighLoc, 0.9, 0.4, 1.0); // Bright purple/pink

		// Build vertex data for spectrum bars
		const binWidth = sampleRate / fftSize;
		const vertices: number[] = [];
		const magnitudes: number[] = [];

		// Log frequency points for smooth curve
		const numPoints = 512;
		const logFreqs = generateLogFrequencies(20, 20000, numPoints);

		for (let i = 0; i < numPoints - 1; i++) {
			const freq = logFreqs[i];
			const nextFreq = logFreqs[i + 1];

			// Skip if frequencies are undefined (shouldn't happen with our array)
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

			const x1 = freqToX(freq, width);
			const x2 = freqToX(nextFreq, width);
			const y1 = height; // Bottom
			const y2 = dbToY(db, height, MIN_DB, MAX_DB);

			// Create a filled rectangle (two triangles)
			// Triangle 1
			vertices.push(x1, y1, x1, y2, x2, y2);
			// Triangle 2
			vertices.push(x1, y1, x2, y2, x2, y1);

			// All vertices get same magnitude for coloring
			const normalizedMag = db;
			for (let j = 0; j < 6; j++) {
				magnitudes.push(normalizedMag);
			}
		}

		// Upload vertex data
		const positionLoc = gl.getAttribLocation(program, "a_position");
		const magnitudeLoc = gl.getAttribLocation(program, "a_magnitude");

		// Position buffer
		gl.bindBuffer(gl.ARRAY_BUFFER, positionBufferRef.current);
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(vertices), gl.DYNAMIC_DRAW);
		gl.enableVertexAttribArray(positionLoc);
		gl.vertexAttribPointer(positionLoc, 2, gl.FLOAT, false, 0, 0);

		// Magnitude buffer (create temporary)
		const magBuffer = gl.createBuffer();
		gl.bindBuffer(gl.ARRAY_BUFFER, magBuffer);
		gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(magnitudes), gl.DYNAMIC_DRAW);
		gl.enableVertexAttribArray(magnitudeLoc);
		gl.vertexAttribPointer(magnitudeLoc, 1, gl.FLOAT, false, 0, 0);

		// Draw
		gl.drawArrays(gl.TRIANGLES, 0, vertices.length / 2);

		// Cleanup
		gl.deleteBuffer(magBuffer);

		// Draw grid lines (simple lines overlay)
		drawGrid(gl, width, height);
	}, [spectrumData, sampleRate, fftSize]);

	// Draw grid lines (uses SVG overlay instead of WebGL for simplicity)
	const drawGrid = (_gl: WebGLRenderingContext, _width: number, _height: number) => {
		// Grid rendering is handled by the SVG overlay below
		// A WebGL implementation would require a separate shader program
	};

	// Handle resize
	useEffect(() => {
		const canvas = canvasRef.current;
		if (!canvas) return;

		const resizeObserver = new ResizeObserver((entries) => {
			for (const entry of entries) {
				const { width, height } = entry.contentRect;
				canvas.width = width * window.devicePixelRatio;
				canvas.height = height * window.devicePixelRatio;
				canvas.style.width = `${width}px`;
				canvas.style.height = `${height}px`;
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

	return (
		<div className="flex flex-col h-full min-h-0">
			<div className="flex items-center justify-between mb-2 flex-shrink-0">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Spectrum Analyzer (WebGL)
				</h2>
				<span className="text-xs text-muted-foreground px-2 py-0.5 rounded bg-green-500/10 text-green-500">
					GPU Accelerated
				</span>
			</div>
			<div className="flex-1 min-h-[280px] relative rounded-lg overflow-hidden bg-[#0d0d12]">
				<canvas ref={canvasRef} className="absolute inset-0 w-full h-full" />
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
								stroke="rgba(255,255,255,0.1)"
								strokeWidth="0.5"
							/>
						</pattern>
					</defs>
					{/* Frequency labels */}
					<text x="5%" y="98%" fill="rgba(255,255,255,0.5)" fontSize="10">
						20
					</text>
					<text x="20%" y="98%" fill="rgba(255,255,255,0.5)" fontSize="10">
						100
					</text>
					<text x="40%" y="98%" fill="rgba(255,255,255,0.5)" fontSize="10">
						1k
					</text>
					<text x="60%" y="98%" fill="rgba(255,255,255,0.5)" fontSize="10">
						5k
					</text>
					<text x="80%" y="98%" fill="rgba(255,255,255,0.5)" fontSize="10">
						10k
					</text>
					<text x="95%" y="98%" fill="rgba(255,255,255,0.5)" fontSize="10">
						20k
					</text>
					{/* dB labels */}
					<text x="1%" y="5%" fill="rgba(255,255,255,0.5)" fontSize="10">
						0dB
					</text>
					<text x="1%" y="30%" fill="rgba(255,255,255,0.5)" fontSize="10">
						-30
					</text>
					<text x="1%" y="55%" fill="rgba(255,255,255,0.5)" fontSize="10">
						-60
					</text>
					<text x="1%" y="80%" fill="rgba(255,255,255,0.5)" fontSize="10">
						-90
					</text>
					<text x="1%" y="97%" fill="rgba(255,255,255,0.5)" fontSize="10">
						-120
					</text>
				</svg>
				{!isRunning && (
					<div className="absolute inset-0 flex items-center justify-center text-muted-foreground bg-background/50 z-10">
						Start audio stream to see spectrum
					</div>
				)}
			</div>
		</div>
	);
};
