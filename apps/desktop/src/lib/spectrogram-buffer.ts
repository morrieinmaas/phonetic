/** dB value shown where no measurement exists yet. */
const SILENCE_DB = -120;

export interface SpectrogramBuffer {
	push: (spectrum: Float32Array) => void;
	/** Column by age: 0 is newest. */
	column: (age: number) => Float32Array;
	readonly length: number;
	readonly historyLength: number;
}

/**
 * Fixed-size ring of spectra. One allocation up front, none per frame — a
 * spectrogram pushes 40+ columns a second and the draw loop must not stutter.
 */
export const createSpectrogramBuffer = (
	binCount: number,
	historyLength: number,
): SpectrogramBuffer => {
	const columns = Array.from({ length: historyLength }, () =>
		new Float32Array(binCount).fill(SILENCE_DB),
	);
	const silence = new Float32Array(binCount).fill(SILENCE_DB);

	let head = 0;
	let filled = 0;

	return {
		push: (spectrum) => {
			if (spectrum.length !== binCount) {
				return;
			}
			columns[head].set(spectrum);
			head = (head + 1) % historyLength;
			filled = Math.min(filled + 1, historyLength);
		},

		column: (age) => {
			if (age < 0 || age >= filled) {
				return silence;
			}
			return columns[(head - 1 - age + historyLength) % historyLength];
		},

		get length() {
			return filled;
		},

		get historyLength() {
			return historyLength;
		},
	};
};
