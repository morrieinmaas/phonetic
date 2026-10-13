import { describe, expect, it } from "bun:test";
import { createSpectrogramBuffer } from "./spectrogram-buffer";

const spectrum = (value: number, bins = 4) => Float32Array.from({ length: bins }, () => value);

describe("spectrogram buffer", () => {
	it("starts empty", () => {
		expect(createSpectrogramBuffer(4, 10).length).toBe(0);
	});

	it("returns the newest column at age 0", () => {
		const buffer = createSpectrogramBuffer(4, 10);
		buffer.push(spectrum(1));
		buffer.push(spectrum(2));
		expect(Array.from(buffer.column(0))).toEqual([2, 2, 2, 2]);
		expect(Array.from(buffer.column(1))).toEqual([1, 1, 1, 1]);
	});

	it("discards the oldest column once full", () => {
		const buffer = createSpectrogramBuffer(4, 3);
		for (const v of [1, 2, 3, 4]) {
			buffer.push(spectrum(v));
		}
		expect(buffer.length).toBe(3);
		expect(Array.from(buffer.column(0))).toEqual([4, 4, 4, 4]);
		expect(Array.from(buffer.column(2))).toEqual([2, 2, 2, 2]);
	});

	it("returns silence for a column that does not exist yet", () => {
		const buffer = createSpectrogramBuffer(4, 10);
		buffer.push(spectrum(1));
		expect(Array.from(buffer.column(5))).toEqual([-120, -120, -120, -120]);
	});

	it("reuses its allocation rather than growing", () => {
		const buffer = createSpectrogramBuffer(4, 3);
		for (let i = 0; i < 1000; i++) {
			buffer.push(spectrum(i));
		}
		expect(buffer.length).toBe(3);
		expect(buffer.historyLength).toBe(3);
	});

	it("ignores a spectrum of the wrong width rather than corrupting the buffer", () => {
		const buffer = createSpectrogramBuffer(4, 3);
		buffer.push(spectrum(1));
		buffer.push(spectrum(9, 7));
		expect(Array.from(buffer.column(0))).toEqual([1, 1, 1, 1]);
		expect(buffer.length).toBe(1);
	});
});
