/** Audio device information */
export interface AudioDevice {
	id: string;
	name: string;
	is_input: boolean;
	is_output: boolean;
	sample_rates: number[];
	channels: number;
}

/** List of available audio devices */
export interface AudioDeviceList {
	inputs: AudioDevice[];
	outputs: AudioDevice[];
	default_input: string | null;
	default_output: string | null;
}

/** Audio backend type */
export type AudioBackend = "cpal" | "core_audio";

/** Current audio engine state */
export interface AudioState {
	is_running: boolean;
	sample_rate: number;
	buffer_size: number;
	input_device: string | null;
	output_device: string | null;
	generator_active: boolean;
	generator_type: string | null;
	backend: AudioBackend;
	backend_available: AudioBackend[];
	/** Whether exclusive (hog) mode is enabled (macOS CoreAudio only) */
	exclusive_mode: boolean;
}

/** Signal generator types */
export type SignalType = "sine" | "white_noise" | "pink_noise" | "sweep";

/** Transfer function measurement data */
export interface TransferFunctionData {
	magnitude_db: number[];
	phase_deg: number[];
	coherence: number[];
	group_delay_ms: number[];
	frequencies: number[];
}

/** Reference curve types available for overlay */
export type ReferenceCurveType =
	| "flat"
	| "iso226_40_phon"
	| "iso226_60_phon"
	| "iso226_80_phon"
	| "x_curve"
	| "house_curve"
	| "a_weighting"
	| "c_weighting";

/** Reference curve info */
export interface ReferenceCurveInfo {
	curve_type: ReferenceCurveType;
	name: string;
	description: string;
}

/** Reference curve data point */
export interface ReferenceCurvePoint {
	frequency: number;
	amplitude_db: number;
}

/** Complete reference curve with metadata */
export interface ReferenceCurve {
	curve_type: ReferenceCurveType;
	name: string;
	description: string;
	points: ReferenceCurvePoint[];
}

/** Stored measurement trace for A/B comparison */
export interface StoredTrace {
	id: string;
	name: string;
	color: string;
	timestamp: number;
	data: TransferFunctionData;
	visible: boolean;
}

/** Trace slot identifiers for quick A/B comparison */
export type TraceSlot = "A" | "B" | "C" | "D";

/** Fractional-octave smoothing applied to the multi-time-window transfer function. */
export type SmoothingFraction =
	| "none"
	| "octave1"
	| "octave3"
	| "octave6"
	| "octave12"
	| "octave24"
	| "octave48";

/** How urgently a rig diagnosis needs the operator's attention. */
export type Severity = "info" | "warning" | "error";

/** A named cause for a bad measurement, produced by the rig diagnostics rules. */
export interface Diagnosis {
	/** Stable identifier, safe to match on in the UI and in tests. */
	code: string;
	severity: Severity;
	/** Written for a system tech standing at front of house, not for a developer. */
	message: string;
	/** Identifier of a one-click fix the UI can offer, if there is one. */
	action: string | null;
	/** Measured delay in samples, present when the diagnosis is about an unapplied delay. */
	delay_samples: number | null;
}

/** Biquad filter kinds the virtual EQ engine supports. */
export type FilterType = "peaking" | "low_shelf" | "high_shelf" | "high_pass" | "low_pass";

/** A single biquad filter, matching the Rust `BiquadFilter` wire format exactly. */
export interface BiquadFilter {
	filter_type: FilterType;
	frequency_hz: number;
	gain_db: number;
	q: number;
	enabled: boolean;
}

/** Trial/licence status, matching the Rust `LicenseState` wire format exactly. */
export type LicenseState =
	| { state: "trial"; days_remaining: number }
	| { state: "licensed"; key_suffix: string }
	| { state: "expired" };

/** Settings captured alongside a saved session */
export interface SessionSettings {
	sample_rate: number;
	fft_size: number;
	multi_window: boolean;
	smoothing: SmoothingFraction;
	delay_samples: number;
	max_averages: number;
}

/** A measurement session: settings plus the traces captured under them */
export interface Session {
	version: number;
	created_at: string;
	name: string;
	settings: SessionSettings;
	traces: StoredTrace[];
}
