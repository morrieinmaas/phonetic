import { invoke } from "@tauri-apps/api/core";
import type {
	AudioBackend,
	AudioDeviceList,
	AudioState,
	ReferenceCurve,
	ReferenceCurveInfo,
	ReferenceCurveType,
	SignalType,
} from "../types/audio";

/** Transfer function measurement data */
export interface TransferFunctionData {
	magnitude_db: number[];
	phase_deg: number[];
	coherence: number[];
	group_delay_ms: number[];
	frequencies: number[];
}

/** Frequency and amplitude validation */
const MIN_FREQUENCY = 20;
const MAX_FREQUENCY = 20000;
const MIN_AMPLITUDE = 0;
const MAX_AMPLITUDE = 1;

/** Valid FFT sizes (powers of 2) */
const VALID_FFT_SIZES = [512, 1024, 2048, 4096, 8192, 16384];

const validateFrequency = (freq: number): void => {
	if (!Number.isFinite(freq) || freq < MIN_FREQUENCY || freq > MAX_FREQUENCY) {
		throw new Error(`Frequency must be between ${MIN_FREQUENCY} and ${MAX_FREQUENCY} Hz`);
	}
};

const validateAmplitude = (amp: number): void => {
	if (!Number.isFinite(amp) || amp < MIN_AMPLITUDE || amp > MAX_AMPLITUDE) {
		throw new Error(`Amplitude must be between ${MIN_AMPLITUDE} and ${MAX_AMPLITUDE}`);
	}
};

const validateFftSize = (size: number): void => {
	if (!VALID_FFT_SIZES.includes(size)) {
		throw new Error(`FFT size must be one of: ${VALID_FFT_SIZES.join(", ")}`);
	}
};

/**
 * Audio API service wrapping Tauri commands with proper typing and validation
 */
export const audioApi = {
	/**
	 * List all available audio input and output devices
	 */
	listDevices: (): Promise<AudioDeviceList> => invoke<AudioDeviceList>("list_audio_devices"),

	/**
	 * Get current audio engine state
	 */
	getState: (): Promise<AudioState> => invoke<AudioState>("get_audio_state"),

	/**
	 * Start audio stream with specified input/output devices
	 * @param inputDeviceId - Device ID for input (use "input:DeviceName" format)
	 * @param outputDeviceId - Device ID for output (use "output:DeviceName" format)
	 */
	startStream: (inputDeviceId?: string, outputDeviceId?: string): Promise<void> =>
		invoke("start_audio_stream", {
			inputDeviceId: inputDeviceId ?? null,
			outputDeviceId: outputDeviceId ?? null,
		}),

	/**
	 * Stop the current audio stream
	 */
	stopStream: (): Promise<void> => invoke("stop_audio_stream"),

	/**
	 * Start the signal generator
	 * @param signalType - Type of signal to generate
	 * @param frequency - Frequency in Hz (20-20000)
	 * @param amplitude - Amplitude (0-1)
	 */
	startGenerator: (signalType: SignalType, frequency: number, amplitude: number): Promise<void> => {
		validateFrequency(frequency);
		validateAmplitude(amplitude);
		return invoke("start_generator", {
			signalType,
			frequency,
			amplitude,
		});
	},

	/**
	 * Stop the signal generator
	 */
	stopGenerator: (): Promise<void> => invoke("stop_generator"),

	/**
	 * Get current FFT spectrum data
	 * @returns Array of magnitude values in dB
	 */
	getSpectrum: (): Promise<number[]> => invoke<number[]>("get_spectrum_data"),

	/**
	 * Get transfer function measurement data
	 * @returns Magnitude, phase, coherence, and group delay arrays
	 */
	getTransferFunction: (): Promise<TransferFunctionData> =>
		invoke<TransferFunctionData>("get_transfer_function_data"),

	/**
	 * Reset transfer function averaging
	 */
	resetAveraging: (): Promise<void> => invoke("reset_averaging"),

	/**
	 * Set FFT size (must be power of 2)
	 * @param size - FFT size (512, 1024, 2048, 4096, 8192, or 16384)
	 */
	setFftSize: (size: number): Promise<void> => {
		validateFftSize(size);
		return invoke("set_fft_size", { size });
	},

	/**
	 * Set audio backend (requires stream to be stopped)
	 * @param backend - Backend to use ("cpal" or "coreaudio")
	 */
	setAudioBackend: (backend: AudioBackend): Promise<void> =>
		invoke("set_audio_backend", { backend }),

	/**
	 * Set exclusive mode (macOS CoreAudio only)
	 * Requires stream to be stopped; takes effect on next stream start
	 * @param enabled - Whether to request exclusive device access
	 */
	setExclusiveMode: (enabled: boolean): Promise<void> => invoke("set_exclusive_mode", { enabled }),

	/**
	 * Get current exclusive mode setting
	 */
	getExclusiveMode: (): Promise<boolean> => invoke<boolean>("get_exclusive_mode"),

	// ========================================================================
	// Reference Curves
	// ========================================================================

	/**
	 * Get list of available reference curves
	 */
	getAvailableReferenceCurves: (): Promise<ReferenceCurveInfo[]> =>
		invoke<ReferenceCurveInfo[]>("get_available_reference_curves"),

	/**
	 * Get a reference curve with specified number of points
	 * @param curveType - Type of reference curve
	 * @param numPoints - Number of frequency points (10-1024)
	 */
	getReferenceCurve: (curveType: ReferenceCurveType, numPoints = 256): Promise<ReferenceCurve> =>
		invoke<ReferenceCurve>("get_reference_curve", { curveType, numPoints }),

	/**
	 * Get a reference curve at specific frequencies
	 * @param curveType - Type of reference curve
	 * @param frequencies - Array of frequencies to sample
	 */
	getReferenceCurveAtFrequencies: (
		curveType: ReferenceCurveType,
		frequencies: number[],
	): Promise<ReferenceCurve> =>
		invoke<ReferenceCurve>("get_reference_curve_at_frequencies", { curveType, frequencies }),
};

export type {
	AudioBackend,
	AudioDevice,
	AudioDeviceList,
	AudioState,
	ReferenceCurve,
	ReferenceCurveInfo,
	ReferenceCurveType,
	SignalType,
	StoredTrace,
	TraceSlot,
} from "../types/audio";
