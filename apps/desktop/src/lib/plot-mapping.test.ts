import { describe, expect, it } from "bun:test";

/**
 * Guards the arithmetic that positions gridlines and axis labels.
 *
 * Two shipped bugs motivated this file, both of which put a confidently wrong
 * number in front of an operator and neither of which any test could see:
 *
 *   - The WebGL analyser drew its labels at hardcoded percentages. 1 kHz was
 *     painted at 40% of the plot width where a 20 Hz–20 kHz log axis puts it at
 *     56.6%, and 5 kHz at 60% against a true 79.9%. Reading a peak against that
 *     axis would name the wrong frequency by nearly an octave.
 *   - The transfer function used uPlot's distr 2 (ordinal) rather than distr 3
 *     (logarithmic), so its ticks landed on whatever frequency happened to sit
 *     at an evenly spaced index.
 *
 * These mirror the mappings in webgl-spectrum.tsx. If those change, these fail.
 */

const freqToPercent = (hz: number, minFreq = 20, maxFreq = 20000): number =>
	((Math.log10(hz) - Math.log10(minFreq)) / (Math.log10(maxFreq) - Math.log10(minFreq))) * 100;

const dbToPercent = (db: number, minDb = -120, maxDb = 0): number =>
	((maxDb - db) / (maxDb - minDb)) * 100;

describe("freqToPercent", () => {
	it("anchors the ends of the axis", () => {
		expect(freqToPercent(20)).toBeCloseTo(0, 6);
		expect(freqToPercent(20000)).toBeCloseTo(100, 6);
	});

	it("places the decades where a log axis actually puts them", () => {
		// The values the old hardcoded labels got wrong.
		expect(freqToPercent(100)).toBeCloseTo(23.29, 1);
		expect(freqToPercent(1000)).toBeCloseTo(56.64, 1);
		expect(freqToPercent(5000)).toBeCloseTo(79.94, 1);
		expect(freqToPercent(10000)).toBeCloseTo(90.0, 1);
	});

	it("rejects the percentages the broken build used", () => {
		// 1 kHz was drawn at 40% and 5 kHz at 60%. Both are far from correct;
		// this test fails if anyone reintroduces eyeballed positions.
		expect(Math.abs(freqToPercent(1000) - 40)).toBeGreaterThan(10);
		expect(Math.abs(freqToPercent(5000) - 60)).toBeGreaterThan(10);
	});

	it("spaces decades evenly, which is what makes it a log axis", () => {
		const decade = freqToPercent(1000) - freqToPercent(100);
		expect(freqToPercent(10000) - freqToPercent(1000)).toBeCloseTo(decade, 6);
	});

	it("is monotonic across the audio band", () => {
		let previous = Number.NEGATIVE_INFINITY;
		for (let hz = 20; hz <= 20000; hz *= 1.1) {
			const pct = freqToPercent(hz);
			expect(pct).toBeGreaterThan(previous);
			previous = pct;
		}
	});
});

describe("dbToPercent", () => {
	it("puts 0 dB at the top and the floor at the bottom", () => {
		expect(dbToPercent(0)).toBeCloseTo(0, 6);
		expect(dbToPercent(-120)).toBeCloseTo(100, 6);
	});

	it("is linear, so equal dB steps are equal distances", () => {
		const step = dbToPercent(-20) - dbToPercent(0);
		expect(dbToPercent(-40) - dbToPercent(-20)).toBeCloseTo(step, 6);
		expect(dbToPercent(-120) - dbToPercent(-100)).toBeCloseTo(step, 6);
	});

	it("rejects the positions the broken build used", () => {
		// -30 was drawn at 30% and -60 at 55%; the truth is 25% and 50%.
		expect(dbToPercent(-30)).toBeCloseTo(25, 6);
		expect(dbToPercent(-60)).toBeCloseTo(50, 6);
	});
});
