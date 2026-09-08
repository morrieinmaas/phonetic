import { describe, expect, it } from "bun:test";
import { binCount, windowLengthMs } from "./fft-window";

describe("windowLengthMs", () => {
	// An FFT of N samples at fs spans N/fs seconds. The inverse — fs/N — is the
	// bin width in hertz, and reporting it as milliseconds gives 23437.5 ms for
	// a 2048-point window at 48 kHz, which is off by a factor of ~550.
	it("is the span of the window, not the bin width", () => {
		expect(windowLengthMs(2048, 48000)).toBeCloseTo(42.667, 2);
		expect(windowLengthMs(4096, 48000)).toBeCloseTo(85.333, 2);
		expect(windowLengthMs(512, 48000)).toBeCloseTo(10.667, 2);
	});

	it("grows with FFT size and shrinks with sample rate", () => {
		expect(windowLengthMs(4096, 48000)).toBeGreaterThan(windowLengthMs(2048, 48000));
		expect(windowLengthMs(2048, 96000)).toBeLessThan(windowLengthMs(2048, 48000));
	});

	it("handles 44.1 kHz, which is not a multiple of the FFT sizes", () => {
		expect(windowLengthMs(2048, 44100)).toBeCloseTo(46.44, 1);
	});

	it("returns 0 rather than Infinity when the sample rate is unknown", () => {
		expect(windowLengthMs(2048, 0)).toBe(0);
	});
});

describe("binCount", () => {
	// A real FFT of N samples yields N/2 + 1 usable bins, not N.
	it("is half the FFT size plus one", () => {
		expect(binCount(2048)).toBe(1025);
		expect(binCount(512)).toBe(257);
		expect(binCount(16384)).toBe(8193);
	});
});
