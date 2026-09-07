/**
 * Wrap-safe interpolation for wrapped phase data (degrees, range (-180, 180]).
 *
 * Phase is a circular quantity: +179 deg and -179 deg are 2 deg apart, not 358 deg apart.
 * Blending wrapped values directly (as plain linear interpolation would) invents a sweep
 * through 0 deg at every wrap boundary that is not present in the measurement. This module
 * fixes that by unwrapping the series into a continuous representation before handing it to
 * the existing log-scale interpolator, then re-wrapping the result for display.
 */

/** Signature shared with the chart's existing log-scale resampler (e.g. `mapToLogScale`). */
export type LogScaleInterpolator = (
	linearData: number[],
	frequencies: number[],
	logFreqs: number[],
) => number[];

/** Wrap a phase value (degrees) into (-180, 180], matching the DSP layer's `atan2` convention. */
export const wrapPhaseDeg = (deg: number): number => {
	let wrapped = deg % 360;
	if (wrapped <= -180) {
		wrapped += 360;
	} else if (wrapped > 180) {
		wrapped -= 360;
	}
	return wrapped;
};

/**
 * Unwrap a wrapped phase series (degrees) into a continuous series with no jump greater than
 * 180 deg between neighbours, so it can be safely interpolated.
 */
export const unwrapPhaseDeg = (phaseDeg: number[]): number[] => {
	const first = phaseDeg[0];
	if (first === undefined) {
		return [];
	}

	const unwrapped: number[] = [first];
	let offset = 0;
	let previousRaw = first;

	for (let i = 1; i < phaseDeg.length; i++) {
		const raw = phaseDeg[i];
		if (raw === undefined) {
			continue;
		}
		const delta = raw - previousRaw;
		if (delta > 180) {
			offset -= 360;
		} else if (delta < -180) {
			offset += 360;
		}
		unwrapped.push(raw + offset);
		previousRaw = raw;
	}

	return unwrapped;
};

/**
 * Interpolate a wrapped phase series onto a target frequency grid without inventing a sweep
 * through 0 deg at wrap boundaries.
 *
 * The numeric resampling itself is delegated to `interpolate` (the chart's existing log-scale
 * helper) so this only adds the angular handling: unwrap the phase into a continuous series,
 * resample that with the existing helper, then wrap the result back into (-180, 180] for
 * display.
 */
export const interpolatePhaseToLogScale = (
	phaseDeg: number[],
	frequencies: number[],
	logFreqs: number[],
	interpolate: LogScaleInterpolator,
): number[] => {
	const unwrapped = unwrapPhaseDeg(phaseDeg);
	const interpolated = interpolate(unwrapped, frequencies, logFreqs);
	return interpolated.map(wrapPhaseDeg);
};
