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

/** Where a measurement pair takes its reference from. */
export type ReferenceSource = { source: "generator" } | { source: "input"; channel: number };

/** One measurement pair, matching the Rust `MeasurementPair` wire format exactly. */
export interface MeasurementPair {
	id: string;
	name: string;
	measurement_channel: number;
	reference: ReferenceSource;
	delay_samples: number;
	calibration_offset_db: number;
	enabled: boolean;
}

/** The full channel routing. */
export interface Routing {
	pairs: MeasurementPair[];
}

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

/**
 * LEQ/LAeq/LCeq/peak reading, integrated since the stream last (re)started.
 * Matches the Rust `LeqMetrics` wire format exactly.
 *
 * Values arriving over `get_leq_metrics` are calibrated **dB SPL**, converted
 * from the meter's raw dBFS by the command using the app's live SplCalculator.
 * That conversion is not cosmetic: these numbers are rendered beside an
 * absolute noise limit, and raw dBFS next to an 85 dB limit reads as huge
 * headroom when the rig may be well over it.
 */
export interface LeqMetrics {
	leq_db: number;
	laeq_db: number;
	lceq_db: number;
	lafmax_db: number;
	lapeak_db: number;
	lcpeak_db: number;
	elapsed_seconds: number;
}

/**
 * One noise-exposure limit preset, matching the Rust `NoiseLimit` wire
 * format exactly. `source` and `as_of` are required and non-empty on every
 * shipped preset -- a limit without a citation is a number presented as
 * authority, which this app never does.
 */
export interface NoiseLimit {
	id: string;
	name: string;
	source: string;
	as_of: string;
	laeq_limit_db: number;
	reference_hours: number;
	lcpeak_limit_db: number | null;
}

/**
 * The result of comparing a measurement against a limit's reference period,
 * matching the Rust `DoseResult` wire format exactly.
 */
export interface DoseResult {
	percent: number;
	projected_percent_at_reference: number;
	peak_exceeded: boolean;
}

/** Room criteria curve families -- the wire format for `get_room_criteria`. */
export type CurveFamily = "nc" | "nr";

/**
 * Every family selectable in the Room Criteria panel: the two tangency curve
 * families plus RC Mark II, which is not a `CurveFamily` on the Rust side
 * (see `RcResult`) but shares the same selector UI.
 */
export type RoomCriteriaFamily = CurveFamily | "rc";

/** A room criteria rating, matching the Rust `RoomCriteriaResult` wire format exactly. */
export interface RoomCriteriaResult {
	family: CurveFamily;
	rating: number;
	exceeded_band_hz: number;
	band_levels_db: number[];
	curve_db: number[];
}

/**
 * RC Mark II spectral quality descriptor, matching the Rust `RcQuality` wire
 * format exactly. Serde's `rename_all = "snake_case"` turns `LowFrequency`
 * into `low_frequency` and so on.
 */
export type RcQuality = "neutral" | "low_frequency" | "mid_frequency" | "high_frequency";

/**
 * An RC Mark II rating, matching the Rust `RcResult` wire format exactly. Not
 * a tangency rating like `RoomCriteriaResult` -- `level` is the 500/1000/2000 Hz
 * average and `quality` is a separate spectral-shape descriptor.
 */
export interface RcResult {
	level: number;
	quality: RcQuality;
	band_levels_db: number[];
	bins_per_band: number[];
	reference_db: number[];
	under_resolved: boolean;
	/** Minimum FFT bins a band needs to be trustworthy. Sent with the rating so
	 *  the display cannot drift from the threshold the backend actually used. */
	min_bins_per_band: number;
}
