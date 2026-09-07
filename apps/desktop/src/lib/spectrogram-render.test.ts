import { describe, expect, it } from "bun:test";
import { createSpectrogramBuffer } from "./spectrogram-buffer";
import { fillSpectrogramPixels } from "./spectrogram-render";

const MIN_DB = -100;
const MAX_DB = 0;

/** LUT where the red channel equals the LUT index (0-255), so a pixel's red channel
 * tells us exactly which LUT entry (and therefore which dB value) produced it. */
const identityLut = (): Uint8ClampedArray => {
	const lut = new Uint8ClampedArray(256 * 4);
	for (let i = 0; i < 256; i++) {
		lut[i * 4 + 0] = i;
		lut[i * 4 + 1] = 0;
		lut[i * 4 + 2] = 0;
		lut[i * 4 + 3] = 255;
	}
	return lut;
};

const redAt = (target: Uint8ClampedArray, width: number, x: number, y: number): number => {
	const pixelIndex = (y * width + x) * 4;
	return target[pixelIndex] ?? -1;
};

describe("fillSpectrogramPixels", () => {
	it("puts the newest column on the right and the oldest on the left", () => {
		const width = 3;
		const height = 2;
		const buffer = createSpectrogramBuffer(height, width);
		// Oldest to newest: -100dB, -50dB, 0dB -> LUT red 0, 127, 255.
		buffer.push(Float32Array.from([-100, -100]));
		buffer.push(Float32Array.from([-50, -50]));
		buffer.push(Float32Array.from([0, 0]));

		const target = new Uint8ClampedArray(width * height * 4);
		fillSpectrogramPixels(target, buffer, identityLut(), width, height, MIN_DB, MAX_DB, 0);

		const oldestRed = redAt(target, width, 0, 0);
		const newestRed = redAt(target, width, width - 1, 0);
		expect(oldestRed).toBe(0);
		expect(newestRed).toBe(255);
	});

	it("puts low frequency bins at the bottom and high frequency bins at the top", () => {
		const width = 1;
		const height = 3;
		const buffer = createSpectrogramBuffer(height, width);
		// bin 0 = low freq = -100dB, bin 2 = high freq = 0dB.
		buffer.push(Float32Array.from([-100, -50, 0]));

		const target = new Uint8ClampedArray(width * height * 4);
		fillSpectrogramPixels(target, buffer, identityLut(), width, height, MIN_DB, MAX_DB, 0);

		const bottomRed = redAt(target, width, 0, height - 1);
		const topRed = redAt(target, width, 0, 0);
		expect(bottomRed).toBe(0);
		expect(topRed).toBe(255);
	});

	it("maps a magnitude at the floor to the LUT's first colour and at the ceiling to its last", () => {
		const width = 1;
		const height = 2;
		const buffer = createSpectrogramBuffer(height, width);
		const lut = identityLut();
		lut[0 * 4 + 0] = 9;
		lut[0 * 4 + 1] = 8;
		lut[0 * 4 + 2] = 7;
		lut[255 * 4 + 0] = 19;
		lut[255 * 4 + 1] = 18;
		lut[255 * 4 + 2] = 17;
		buffer.push(Float32Array.from([MIN_DB, MAX_DB]));

		const target = new Uint8ClampedArray(width * height * 4);
		fillSpectrogramPixels(target, buffer, lut, width, height, MIN_DB, MAX_DB, 0);

		// bin 0 (floor) ends up at the bottom row, bin 1 (ceiling) at the top row.
		const floorIndex = (height - 1) * width * 4;
		const ceilingIndex = 0 * width * 4;
		expect([target[floorIndex], target[floorIndex + 1], target[floorIndex + 2]]).toEqual([9, 8, 7]);
		expect([target[ceilingIndex], target[ceilingIndex + 1], target[ceilingIndex + 2]]).toEqual([
			19, 18, 17,
		]);
	});

	it("leaves a wrong-length target untouched instead of throwing or writing out of bounds", () => {
		const width = 4;
		const height = 4;
		const buffer = createSpectrogramBuffer(height, width);
		buffer.push(Float32Array.from([-10, -20, -30, -40]));

		const sentinel = 7;
		const target = new Uint8ClampedArray(width * height * 4 - 1).fill(sentinel);

		expect(() =>
			fillSpectrogramPixels(target, buffer, identityLut(), width, height, MIN_DB, MAX_DB, 0),
		).not.toThrow();
		expect(Array.from(target).every((v) => v === sentinel)).toBe(true);
	});
});
