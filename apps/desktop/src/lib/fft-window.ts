/**
 * Relationships between FFT size, sample rate and what the operator reads.
 *
 * These are easy to invert and the inversion is not obvious on screen: showing
 * the bin width where the window length belongs reported a 2048-point window at
 * 48 kHz as 23437.5 ms instead of 42.7 ms. In a measurement tool a wrong number
 * presented confidently is worse than no number.
 */

/** How much time one FFT frame spans, in milliseconds. */
export const windowLengthMs = (fftSize: number, sampleRate: number): number => {
	if (sampleRate <= 0) {
		return 0;
	}
	return (fftSize / sampleRate) * 1000;
};

/** Usable frequency bins produced by a real FFT of `fftSize` samples. */
export const binCount = (fftSize: number): number => fftSize / 2 + 1;
