/**
 * Perceptually monotonic dark-to-bright ramp (near-black → blue → magenta →
 * orange → white). Monotonic brightness matters: an operator reads level by
 * brightness, and a ramp that dips mid-scale invents features that are not there.
 */
const STOPS: [number, number, number][] = [
	[0, 0, 12],
	[42, 12, 110],
	[135, 22, 130],
	[218, 68, 78],
	[252, 158, 44],
	[255, 255, 220],
];

export const magnitudeToRgb = (
	db: number,
	minDb: number,
	maxDb: number,
): [number, number, number] => {
	const span = maxDb - minDb;
	const t = span <= 0 ? 0 : Math.min(1, Math.max(0, (db - minDb) / span));

	const scaled = t * (STOPS.length - 1);
	const index = Math.min(STOPS.length - 2, Math.floor(scaled));
	const frac = scaled - index;

	const [r0, g0, b0] = STOPS[index];
	const [r1, g1, b1] = STOPS[index + 1];

	return [
		Math.round(r0 + (r1 - r0) * frac),
		Math.round(g0 + (g1 - g0) * frac),
		Math.round(b0 + (b1 - b0) * frac),
	];
};
