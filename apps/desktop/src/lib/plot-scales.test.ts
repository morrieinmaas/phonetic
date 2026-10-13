import { describe, expect, it } from "bun:test";
import {
	dbSplits,
	exactRange,
	formatDelta,
	formatFrequencyTick,
	logFrequencyToPixel,
	nearestIndex,
	pixelToLogFrequency,
} from "./plot-scales";
import { applyPan, applyZoom } from "./plot-scales";

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

describe("applyZoom", () => {
	const limits: [number, number] = [20, 20000];

	it("narrows the range when zooming in", () => {
		const [lo, hi] = applyZoom([20, 20000], 0.5, 1000, limits);
		expect(hi - lo).toBeLessThan(19980);
	});

	it("keeps the anchor frequency under the pointer", () => {
		const before: [number, number] = [100, 10000];
		const [lo, hi] = applyZoom(before, 0.5, 1000, limits);
		expect(lo).toBeLessThan(1000);
		expect(hi).toBeGreaterThan(1000);
	});

	it("never zooms out past the limits", () => {
		const [lo, hi] = applyZoom([20, 20000], 100, 1000, limits);
		expect(lo).toBeGreaterThanOrEqual(20);
		expect(hi).toBeLessThanOrEqual(20000);
	});

	it("refuses to invert the range", () => {
		const [lo, hi] = applyZoom([1000, 1001], 0.0001, 1000, limits);
		expect(hi).toBeGreaterThan(lo);
	});
});

describe("applyPan", () => {
	const limits: [number, number] = [20, 20000];

	it("shifts the window", () => {
		const [lo] = applyPan([100, 1000], 0.1, limits);
		expect(lo).toBeGreaterThan(100);
	});

	it("stops at the limits instead of scrolling into nothing", () => {
		const [lo, hi] = applyPan([20, 200], -1, limits);
		expect(lo).toBeGreaterThanOrEqual(20);
		expect(hi - lo).toBeCloseTo(180, 0);
	});
});

describe("frequency axis helpers", () => {
	it("labels the 1-2-5 series and blanks filtered ticks", () => {
		expect([20, 500, 1000, 20000, null].map(formatFrequencyTick)).toEqual([
			"20",
			"500",
			"1k",
			"20k",
			"",
		]);
	});

	it("passes the requested range through instead of widening to decades", () => {
		expect(exactRange(null, 20, 20000)).toEqual([20, 20000]);
	});

	it("steps dB ticks to fit the range", () => {
		expect(dbSplits(-40, 20)).toEqual([-40, -30, -20, -10, 0, 10, 20]);
		expect(dbSplits(-10, 5)).toEqual([-10, -5, 0, 5]);
		expect(dbSplits(-3, 3)).toEqual([-2, 0, 2]);
	});
});
