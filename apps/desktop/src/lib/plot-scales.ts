export interface CursorSample {
	frequencyHz: number;
	magnitudeDb: number;
	groupDelayMs: number;
}

/** Pixel column to frequency on a log axis. */
export const pixelToLogFrequency = (
	x: number,
	width: number,
	fMin: number,
	fMax: number,
): number => {
	if (width <= 0) {
		return fMin;
	}
	const t = Math.min(1, Math.max(0, x / width));
	return fMin * (fMax / fMin) ** t;
};

/** Frequency to pixel column on a log axis. */
export const logFrequencyToPixel = (
	freq: number,
	width: number,
	fMin: number,
	fMax: number,
): number => {
	if (width <= 0 || freq <= 0) {
		return 0;
	}
	return (Math.log(freq / fMin) / Math.log(fMax / fMin)) * width;
};

/** Index of the closest frequency, or -1 if there is no axis. */
export const nearestIndex = (frequencies: number[], target: number): number => {
	if (frequencies.length === 0) {
		return -1;
	}
	let best = 0;
	let bestDistance = Number.POSITIVE_INFINITY;
	for (let i = 0; i < frequencies.length; i++) {
		const freq = frequencies[i];
		if (freq === undefined) {
			continue;
		}
		const distance = Math.abs(freq - target);
		if (distance < bestDistance) {
			best = i;
			bestDistance = distance;
		}
	}
	return best;
};

/** Signed differences between two cursors, b relative to a. */
export const formatDelta = (a: CursorSample, b: CursorSample) => ({
	deltaFrequencyHz: b.frequencyHz - a.frequencyHz,
	deltaDb: b.magnitudeDb - a.magnitudeDb,
	deltaMs: b.groupDelayMs - a.groupDelayMs,
});

type Range = [number, number];

/** Zoom about `anchor`, clamped so the view never leaves `limits` or inverts. */
export const applyZoom = (range: Range, factor: number, anchor: number, limits: Range): Range => {
	const [lo, hi] = range;
	const width = hi - lo;
	const newWidth = Math.max(1e-3, width * factor);

	// Keep the anchor at the same fractional position in the view.
	const position = width === 0 ? 0.5 : (anchor - lo) / width;
	let newLo = anchor - position * newWidth;
	let newHi = newLo + newWidth;

	if (newLo < limits[0]) {
		newHi += limits[0] - newLo;
		newLo = limits[0];
	}
	if (newHi > limits[1]) {
		newLo -= newHi - limits[1];
		newHi = limits[1];
	}

	return [Math.max(limits[0], newLo), Math.min(limits[1], newHi)];
};

/** Pan by a fraction of the visible width, clamped to `limits`. */
export const applyPan = (range: Range, deltaFraction: number, limits: Range): Range => {
	const [lo, hi] = range;
	const width = hi - lo;
	let newLo = lo + width * deltaFraction;

	newLo = Math.max(limits[0], Math.min(limits[1] - width, newLo));
	return [newLo, newLo + width];
};

/** Tick positions for a 20 Hz to 20 kHz log axis: the 1-2-5 series an engineer reads. */
export const FREQUENCY_SPLITS = [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000];

/** "20", "500", "1k", "20k". Null ticks (filtered out by uPlot) stay blank. */
export const formatFrequencyTick = (value: number | null): string => {
	if (value === null) return "";
	return value >= 1000 ? `${value / 1000}k` : String(Math.round(value));
};

/**
 * Scale range that returns exactly what it is given. uPlot's default for a log
 * scale widens to whole decades, which drew 20 Hz to 20 kHz as 20 Hz to 100 kHz
 * and left a fifth of every plot empty.
 */
export const exactRange = (_u: unknown, min: number, max: number): [number, number] => [min, max];

/** Keep every split. uPlot's log-axis filter blanks all but powers of ten. */
export const keepAllSplits = (_u: unknown, splits: number[]): number[] => splits;

/** Evenly stepped dB ticks for whatever range auto-zoom picked. */
export const dbSplits = (min: number, max: number): number[] => {
	const span = max - min;
	const step = span > 60 ? 20 : span > 30 ? 10 : span > 12 ? 5 : 2;
	const first = Math.ceil(min / step) * step;
	return Array.from({ length: Math.floor((max - first) / step) + 1 }, (_, i) => first + i * step);
};
