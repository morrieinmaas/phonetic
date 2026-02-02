import { create } from "zustand";
import {
	type AudioDeviceList,
	type AudioState,
	type SignalType,
	type TransferFunctionData,
	audioApi,
} from "../services/audio-api";

interface AudioStore {
	// Device state
	devices: AudioDeviceList | null;
	selectedInputDevice: string | null;
	selectedOutputDevice: string | null;

	// Audio engine state
	audioState: AudioState | null;
	isLoading: boolean;
	error: string | null;

	// Generator state
	generatorFrequency: number;
	generatorAmplitude: number;
	generatorType: SignalType;

	// Spectrum data
	spectrumData: number[];
	transferFunctionData: TransferFunctionData | null;

	// FFT settings
	fftSize: number;

	// Actions
	fetchDevices: () => Promise<void>;
	fetchAudioState: () => Promise<void>;
	setSelectedInputDevice: (deviceId: string | null) => void;
	setSelectedOutputDevice: (deviceId: string | null) => void;
	startStream: () => Promise<void>;
	stopStream: () => Promise<void>;
	startGenerator: () => Promise<void>;
	stopGenerator: () => Promise<void>;
	setGeneratorFrequency: (frequency: number) => void;
	setGeneratorAmplitude: (amplitude: number) => void;
	setGeneratorType: (type: SignalType) => void;
	fetchSpectrum: () => Promise<void>;
	fetchTransferFunction: () => Promise<void>;
	resetAveraging: () => Promise<void>;
	setFftSize: (size: number) => Promise<void>;
	clearError: () => void;
}

export const useAudioStore = create<AudioStore>((set, get) => ({
	// Initial state
	devices: null,
	selectedInputDevice: null,
	selectedOutputDevice: null,
	audioState: null,
	isLoading: false,
	error: null,
	generatorFrequency: 1000,
	generatorAmplitude: 0.5,
	generatorType: "sine",
	spectrumData: [],
	transferFunctionData: null,
	fftSize: 2048,

	// Actions
	fetchDevices: async () => {
		set({ isLoading: true, error: null });
		try {
			const devices = await audioApi.listDevices();
			set({
				devices,
				selectedInputDevice: devices.default_input,
				selectedOutputDevice: devices.default_output,
				isLoading: false,
			});
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e), isLoading: false });
		}
	},

	fetchAudioState: async () => {
		try {
			const audioState = await audioApi.getState();
			set({ audioState });
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e) });
		}
	},

	setSelectedInputDevice: (deviceId) => set({ selectedInputDevice: deviceId }),
	setSelectedOutputDevice: (deviceId) => set({ selectedOutputDevice: deviceId }),

	startStream: async () => {
		const { selectedInputDevice, selectedOutputDevice } = get();
		set({ isLoading: true, error: null });
		try {
			await audioApi.startStream(
				selectedInputDevice ?? undefined,
				selectedOutputDevice ?? undefined,
			);
			await get().fetchAudioState();
			set({ isLoading: false });
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e), isLoading: false });
		}
	},

	stopStream: async () => {
		set({ isLoading: true, error: null });
		try {
			await audioApi.stopStream();
			await get().fetchAudioState();
			set({ isLoading: false });
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e), isLoading: false });
		}
	},

	startGenerator: async () => {
		const { generatorType, generatorFrequency, generatorAmplitude } = get();
		set({ error: null });
		try {
			await audioApi.startGenerator(generatorType, generatorFrequency, generatorAmplitude);
			await get().fetchAudioState();
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e) });
		}
	},

	stopGenerator: async () => {
		set({ error: null });
		try {
			await audioApi.stopGenerator();
			await get().fetchAudioState();
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e) });
		}
	},

	setGeneratorFrequency: (frequency) => set({ generatorFrequency: frequency }),
	setGeneratorAmplitude: (amplitude) => set({ generatorAmplitude: amplitude }),
	setGeneratorType: (type) => set({ generatorType: type }),

	fetchSpectrum: async () => {
		try {
			const spectrumData = await audioApi.getSpectrum();
			set({ spectrumData });
		} catch (_e) {
			// Silent fail for spectrum polling - don't spam errors
		}
	},

	fetchTransferFunction: async () => {
		try {
			const transferFunctionData = await audioApi.getTransferFunction();
			set({ transferFunctionData });
		} catch (_e) {
			// Silent fail for transfer function polling
		}
	},

	resetAveraging: async () => {
		set({ error: null });
		try {
			await audioApi.resetAveraging();
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e) });
		}
	},

	setFftSize: async (size) => {
		set({ error: null });
		try {
			await audioApi.setFftSize(size);
			set({ fftSize: size });
		} catch (e) {
			set({ error: e instanceof Error ? e.message : String(e) });
		}
	},

	clearError: () => set({ error: null }),
}));
