import { audioApi } from "./audio-api";

/** Subset of the delay finder's result needed to decide whether to apply it. */
interface MeasuredDelay {
	delay_samples: number;
	is_valid: boolean;
}

/** Apply a measured delay, but only if the finder was confident in it. */
export const applyMeasuredDelay = async (
	delay: MeasuredDelay,
	setDelay: (samples: number) => Promise<void> = audioApi.setTransferFunctionDelay,
): Promise<boolean> => {
	if (!delay.is_valid || delay.delay_samples < 0) {
		return false;
	}
	await setDelay(delay.delay_samples);
	return true;
};
