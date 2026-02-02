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

/** Current audio engine state */
export interface AudioState {
	is_running: boolean;
	sample_rate: number;
	buffer_size: number;
	input_device: string | null;
	output_device: string | null;
	generator_active: boolean;
	generator_type: string | null;
}

/** Signal generator types */
export type SignalType = "sine" | "white_noise" | "pink_noise" | "sweep";
