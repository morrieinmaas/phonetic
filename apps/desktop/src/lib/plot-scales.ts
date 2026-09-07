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
	let bestDistance = Math.abs(frequencies[0] - target);
	for (let i = 1; i < frequencies.length; i++) {
		const distance = Math.abs(frequencies[i] - target);
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
