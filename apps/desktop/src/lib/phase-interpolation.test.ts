import { describe, expect, it } from "bun:test";
import {
	type LogScaleInterpolator,
	interpolatePhaseToLogScale,
	wrapPhaseDeg,
} from "./phase-interpolation";

/**
 * Plain linear interpolation between the two frequency-bracketing samples. This is the "wrong"
 * approach for a circular quantity like phase: fed directly with wrapped values, it blends
 * +179 deg and -179 deg toward 0 deg instead of toward +-180 deg. Used below only to prove that
 * `interpolatePhaseToLogScale` avoids that failure mode, and that it matches naive interpolation
 * whenever there is no wrap to get wrong.
 */
const naiveLinearInterpolate: LogScaleInterpolator = (linearData, frequencies, logFreqs) => {
	return logFreqs.map((target) => {
		if (frequencies.length === 0) {
			return 0;
		}
		if (frequencies.length === 1) {
			return linearData[0] ?? 0;
		}

		let lower = 0;
		while (lower < frequencies.length - 2) {
			const next = frequencies[lower + 1];
			if (next !== undefined && next >= target) {
				break;
			}
			lower++;
		}

		const f0 = frequencies[lower];
		const f1 = frequencies[lower + 1];
		const d0 = linearData[lower];
		const d1 = linearData[lower + 1];
		if (f0 === undefined || f1 === undefined || d0 === undefined || d1 === undefined) {
			return 0;
		}

		const t = f1 === f0 ? 0 : (target - f0) / (f1 - f0);
		return d0 + (d1 - d0) * t;
	});
};

describe("interpolatePhaseToLogScale", () => {
	it("discriminates from naive linear interpolation across a wrap boundary", () => {
		const phaseDeg = [179, -179];
		const frequencies = [1000, 2000];
		const logFreqs = [1000, 1500, 2000];

		const naive = naiveLinearInterpolate(phaseDeg, frequencies, logFreqs);
		const midpointNaive = naive[1];
		expect(midpointNaive).toBeDefined();
		if (midpointNaive === undefined) {
			throw new Error("unreachable");
		}
		// The naive approach blends +179 and -179 straight toward 0, inventing a feature that
		// is not in the measurement.
		expect(Math.abs(midpointNaive)).toBeLessThan(10);

		const correct = interpolatePhaseToLogScale(
			phaseDeg,
			frequencies,
			logFreqs,
			naiveLinearInterpolate,
		);
		expect(correct).toHaveLength(3);
		for (const value of correct) {
			expect(value).toBeDefined();
			if (value === undefined) {
				throw new Error("unreachable");
			}
			// Must never land near 0 deg: the true phase is continuously near +-180 deg here.
			expect(Math.abs(value)).toBeGreaterThan(170);
		}
	});

	it("matches naive linear interpolation for a smooth, non-wrapping ramp", () => {
		const phaseDeg = [-30, -20, -10];
		const frequencies = [100, 200, 300];
		const logFreqs = [100, 150, 200, 250, 300];

		const naive = naiveLinearInterpolate(phaseDeg, frequencies, logFreqs);
		const correct = interpolatePhaseToLogScale(
			phaseDeg,
			frequencies,
			logFreqs,
			naiveLinearInterpolate,
		);

		expect(correct).toHaveLength(naive.length);
		for (let i = 0; i < naive.length; i++) {
			const naiveValue = naive[i];
			const correctValue = correct[i];
			expect(naiveValue).toBeDefined();
			expect(correctValue).toBeDefined();
			if (naiveValue === undefined || correctValue === undefined) {
				throw new Error("unreachable");
			}
			expect(correctValue).toBeCloseTo(naiveValue, 6);
		}
	});

	it("leaves a constant series constant", () => {
		const phaseDeg = [45, 45, 45, 45];
		const frequencies = [100, 200, 300, 400];
		const logFreqs = [100, 150, 250, 400];

		const result = interpolatePhaseToLogScale(
			phaseDeg,
			frequencies,
			logFreqs,
			naiveLinearInterpolate,
		);

		expect(result).toHaveLength(4);
		for (const value of result) {
			expect(value).toBeCloseTo(45, 6);
		}
	});

	it("returns empty for empty input without throwing", () => {
		expect(() => interpolatePhaseToLogScale([], [], [], naiveLinearInterpolate)).not.toThrow();
		expect(interpolatePhaseToLogScale([], [], [], naiveLinearInterpolate)).toEqual([]);
	});
});

describe("wrapPhaseDeg", () => {
	it("wraps values a full turn or more out of range", () => {
		expect(wrapPhaseDeg(540)).toBeCloseTo(180, 6);
		expect(wrapPhaseDeg(-540)).toBeCloseTo(180, 6);
	});

	it("wraps a value just past +180 to just past -180", () => {
		expect(wrapPhaseDeg(181)).toBeCloseTo(-179, 6);
	});

	it("leaves values already in (-180, 180] untouched", () => {
		expect(wrapPhaseDeg(90)).toBeCloseTo(90, 6);
		expect(wrapPhaseDeg(-90)).toBeCloseTo(-90, 6);
		expect(wrapPhaseDeg(180)).toBeCloseTo(180, 6);
		expect(wrapPhaseDeg(0)).toBeCloseTo(0, 6);
	});
});
