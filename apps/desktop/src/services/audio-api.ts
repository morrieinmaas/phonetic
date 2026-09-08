import { invoke } from "@tauri-apps/api/core";
import type {
	AudioBackend,
	AudioDeviceList,
	AudioState,
	BiquadFilter,
	CurveFamily,
	Diagnosis,
	LicenseState,
	ReferenceCurve,
	ReferenceCurveInfo,
	ReferenceCurveType,
	RoomCriteriaResult,
	Session,
	SignalType,
	SmoothingFraction,
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
	 * Delay the transfer function reference to compensate for propagation time
	 * @param delaySamples - Reference delay in samples
	 */
	setTransferFunctionDelay: (delaySamples: number): Promise<void> =>
		invoke("set_transfer_function_delay", { delaySamples }),

	/**
	 * Get the current transfer function reference delay
	 * @returns Reference delay in samples
	 */
	getTransferFunctionDelay: (): Promise<number> => invoke<number>("get_transfer_function_delay"),

	/**
	 * Set the number of frames retained by the transfer function running average
	 * @param count - Averaging depth in frames
	 */
	setMaxAverages: (count: number): Promise<void> => invoke("set_max_averages", { count }),

	/**
	 * Switch between the single-FFT processor and the multi-time-window one.
	 * A fixed FFT cannot serve the whole band: 4096 points at 48 kHz gives
	 * 11.7 Hz resolution, adequate at 10 kHz and useless at 60 Hz.
	 * @param multiWindow - true for multi-time-window, false for a single FFT
	 */
	setResolutionMode: (multiWindow: boolean): Promise<void> =>
		invoke("set_resolution_mode", { multiWindow }),

	/**
	 * Whether the multi-time-window processor is currently active
	 */
	isMultiWindow: (): Promise<boolean> => invoke<boolean>("is_multi_window"),

	/**
	 * Set fractional-octave smoothing on the multi-time-window output
	 * @param fraction - Smoothing width, or "none"
	 */
	setSmoothing: (fraction: SmoothingFraction): Promise<void> =>
		invoke("set_smoothing", { fraction }),

	/**
	 * Set audio backend (requires stream to be stopped)
	 * @param backend - Backend to use ("cpal" or "core_audio")
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

	/**
	 * Get the rig's current diagnoses, most severe first.
	 */
	getDiagnostics: (): Promise<Diagnosis[]> => invoke<Diagnosis[]>("get_diagnostics"),

	/**
	 * Configure how many input channels the next stream should open.
	 * Clamped server-side to 1-8; takes effect on the next stream start.
	 * @param count - Number of input channels to open
	 */
	setInputChannelCount: (count: number): Promise<void> =>
		invoke("set_input_channel_count", { count }),

	/**
	 * Select which input channel feeds level metering and rig diagnostics.
	 * @param index - Zero-based channel index
	 */
	setMeasurementChannel: (index: number): Promise<void> =>
		invoke("set_measurement_channel", { index }),

	/**
	 * Average several measurement positions into one trace, in the complex
	 * domain so cancellations show up rather than being averaged away.
	 * @param traces - Transfer function data from each position, sharing one frequency axis
	 */
	averageTraces: (traces: TransferFunctionData[]): Promise<TransferFunctionData> =>
		invoke<TransferFunctionData>("average_traces", { traces }),

	/**
	 * Save a measurement session as pretty-printed JSON at the given path
	 * @param path - Filesystem path to write the session to
	 * @param session - The session document to persist
	 */
	saveSession: (path: string, session: Session): Promise<void> =>
		invoke("save_session", { path, session }),

	/**
	 * Load a measurement session from disk
	 * @param path - Filesystem path to read the session from
	 */
	loadSession: (path: string): Promise<Session> => invoke<Session>("load_session", { path }),

	// ========================================================================
	// Virtual EQ
	// ========================================================================

	/**
	 * Predict the effect of a filter bank on a measured transfer function.
	 * Coherence is carried through untouched by the backend.
	 * @param trace - The measured transfer function to apply the filter bank to
	 * @param filters - The filter bank, in series
	 * @param sampleRate - Sample rate the filters' coefficients are computed against
	 */
	predictEq: (
		trace: TransferFunctionData,
		filters: BiquadFilter[],
		sampleRate: number,
	): Promise<TransferFunctionData> =>
		invoke<TransferFunctionData>("predict_eq", { trace, filters, sampleRate }),

	// ========================================================================
	// Room Criteria
	// ========================================================================

	/**
	 * Rate the current calibrated spectrum against an NC/NR curve family.
	 * @param family - The curve family to rate against
	 */
	getRoomCriteria: (family: CurveFamily): Promise<RoomCriteriaResult> =>
		invoke<RoomCriteriaResult>("get_room_criteria", { family }),

	// ========================================================================
	// Licensing
	// ========================================================================

	/**
	 * Get the current trial/licence state, persisting the first-run date on first call.
	 */
	getLicenseState: (): Promise<LicenseState> => invoke<LicenseState>("get_license_state"),

	/**
	 * Activate a licence key with the licence server.
	 * @param key - The licence key to activate
	 */
	activateLicense: (key: string): Promise<LicenseState> =>
		invoke<LicenseState>("activate_license", { key }),
};

/**
 * Measure a transfer function from two channels of a WAV file (offline analysis)
 * @param path - Filesystem path to the WAV file
 * @param referenceChannel - Index of the reference channel
 * @param measurementChannel - Index of the measurement channel
 */
export const measureWavFile = (
	path: string,
	referenceChannel: number,
	measurementChannel: number,
): Promise<TransferFunctionData> =>
	invoke("measure_wav_file", { path, referenceChannel, measurementChannel });

export type {
	AudioBackend,
	AudioDevice,
	AudioDeviceList,
	AudioState,
	BiquadFilter,
	CurveFamily,
	FilterType,
	LicenseState,
	ReferenceCurve,
	ReferenceCurveInfo,
	ReferenceCurveType,
	RoomCriteriaResult,
	Session,
	SessionSettings,
	SignalType,
	StoredTrace,
	TraceSlot,
} from "../types/audio";
