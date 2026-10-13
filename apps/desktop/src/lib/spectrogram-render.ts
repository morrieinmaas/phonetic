import type { SpectrogramBuffer } from "./spectrogram-buffer";

/** dB value used for a bin with no data yet, matching spectrogram-buffer's silence fill. */
const NO_DATA_DB = -120;

/**
 * Fills `target` (an ImageData-backed Uint8ClampedArray) with the current spectrogram
 * waterfall: columns run oldest (left) to newest (right), bins run bottom (low
 * frequency) to top (high frequency), and colour comes from `lut` indexed by the
 * dB value normalized into [minDb, maxDb].
 *
 * Pure and allocation-free — this is the per-frame draw path and must not stutter.
 * If `target` isn't sized for exactly `width * height * 4` RGBA bytes, it is left
 * untouched rather than risking an out-of-bounds or partially-written frame.
 */
export const fillSpectrogramPixels = (
	target: Uint8ClampedArray,
	buffer: SpectrogramBuffer,
	lut: Uint8ClampedArray,
	width: number,
	height: number,
	minDb: number,
	maxDb: number,
	background: number,
): void => {
	if (target.length !== width * height * 4) {
		return;
	}

	const dbRange = maxDb - minDb;

	for (let x = 0; x < width; x++) {
		const age = width - 1 - x;
		const hasColumn = age < buffer.length;
		const column = buffer.column(age);

		for (let row = 0; row < height; row++) {
			const y = height - 1 - row;
			const pixelIndex = (y * width + x) * 4;

			if (hasColumn) {
				const db = column[row] ?? NO_DATA_DB;
				// Normalize to 0-1 range
				const normalized = Math.max(0, Math.min(1, (db - minDb) / dbRange));
				// Map to color LUT index (0-255)
				const lutIndex = Math.floor(normalized * 255) * 4;

				target[pixelIndex + 0] = lut[lutIndex + 0] ?? 0;
				target[pixelIndex + 1] = lut[lutIndex + 1] ?? 0;
				target[pixelIndex + 2] = lut[lutIndex + 2] ?? 0;
				target[pixelIndex + 3] = 255;
			} else {
				// Background color for empty areas
				target[pixelIndex + 0] = background;
				target[pixelIndex + 1] = background;
				target[pixelIndex + 2] = background;
				target[pixelIndex + 3] = 255;
			}
		}
	}
};
