import { describe, expect, it } from "bun:test";
import { formatDelta, logFrequencyToPixel, nearestIndex, pixelToLogFrequency } from "./plot-scales";

describe("log frequency mapping", () => {
	it("maps the left edge to f_min and the right edge to f_max", () => {
		expect(pixelToLogFrequency(0, 800, 20, 20000)).toBeCloseTo(20, 3);
		expect(pixelToLogFrequency(800, 800, 20, 20000)).toBeCloseTo(20000, 0);
	});

	it("puts the geometric midpoint at the centre", () => {
		const middle = pixelToLogFrequency(400, 800, 20, 20000);
		expect(middle).toBeCloseTo(Math.sqrt(20 * 20000), 0);
	});

	it("round-trips through the inverse", () => {
		for (const f of [20, 100, 440, 1000, 10000, 20000]) {
			const x = logFrequencyToPixel(f, 800, 20, 20000);
			expect(pixelToLogFrequency(x, 800, 20, 20000)).toBeCloseTo(f, 0);
		}
	});

	it("survives a zero-width plot without returning NaN", () => {
		expect(Number.isFinite(pixelToLogFrequency(0, 0, 20, 20000))).toBe(true);
	});
});

describe("nearestIndex", () => {
	const freqs = [20, 100, 1000, 10000];

	it("finds the closest bin", () => {
		expect(nearestIndex(freqs, 900)).toBe(2);
		expect(nearestIndex(freqs, 21)).toBe(0);
	});

	it("clamps beyond either end", () => {
		expect(nearestIndex(freqs, 1)).toBe(0);
		expect(nearestIndex(freqs, 999999)).toBe(3);
	});

	it("returns -1 for an empty axis", () => {
		expect(nearestIndex([], 100)).toBe(-1);
	});
});

describe("formatDelta", () => {
	it("reports the differences between two cursors", () => {
		const d = formatDelta(
			{ frequencyHz: 100, magnitudeDb: -6, groupDelayMs: 1 },
			{ frequencyHz: 1000, magnitudeDb: -3, groupDelayMs: 4.5 },
		);
		expect(d.deltaFrequencyHz).toBe(900);
		expect(d.deltaDb).toBeCloseTo(3, 5);
		expect(d.deltaMs).toBeCloseTo(3.5, 5);
	});

	it("is signed, so direction is readable", () => {
		const d = formatDelta(
			{ frequencyHz: 1000, magnitudeDb: 0, groupDelayMs: 5 },
			{ frequencyHz: 100, magnitudeDb: -6, groupDelayMs: 1 },
		);
		expect(d.deltaFrequencyHz).toBe(-900);
		expect(d.deltaDb).toBeCloseTo(-6, 5);
	});
});
