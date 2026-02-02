import { beforeEach, describe, expect, it, mock } from "bun:test";

// Mock Tauri's invoke function
const mockInvoke = mock(() => Promise.resolve({}));

mock.module("@tauri-apps/api/core", () => ({
	invoke: mockInvoke,
}));

// Import after mocking
import { useAudioStore } from "./audio-store";

describe("useAudioStore", () => {
	beforeEach(() => {
		mockInvoke.mockClear();
		// Reset store to initial state
		useAudioStore.setState({
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
		});
	});

	describe("initial state", () => {
		it("should have correct initial state", () => {
			const state = useAudioStore.getState();

			expect(state.devices).toBeNull();
			expect(state.selectedInputDevice).toBeNull();
			expect(state.selectedOutputDevice).toBeNull();
			expect(state.audioState).toBeNull();
			expect(state.isLoading).toBe(false);
			expect(state.error).toBeNull();
			expect(state.generatorFrequency).toBe(1000);
			expect(state.generatorAmplitude).toBe(0.5);
			expect(state.generatorType).toBe("sine");
			expect(state.spectrumData).toEqual([]);
			expect(state.transferFunctionData).toBeNull();
			expect(state.fftSize).toBe(2048);
		});
	});

	describe("fetchDevices", () => {
		it("should fetch and store devices", async () => {
			const mockDevices = {
				inputs: [
					{
						id: "input:Test",
						name: "Test",
						is_input: true,
						is_output: false,
						sample_rates: [48000],
						channels: 2,
					},
				],
				outputs: [
					{
						id: "output:Test",
						name: "Test Out",
						is_input: false,
						is_output: true,
						sample_rates: [48000],
						channels: 2,
					},
				],
				default_input: "input:Test",
				default_output: "output:Test",
			};
			mockInvoke.mockResolvedValueOnce(mockDevices);

			await useAudioStore.getState().fetchDevices();

			const state = useAudioStore.getState();
			expect(state.devices).toEqual(mockDevices);
			expect(state.selectedInputDevice).toBe("input:Test");
			expect(state.selectedOutputDevice).toBe("output:Test");
			expect(state.isLoading).toBe(false);
		});

		it("should handle errors", async () => {
			mockInvoke.mockRejectedValueOnce(new Error("Device error"));

			await useAudioStore.getState().fetchDevices();

			const state = useAudioStore.getState();
			expect(state.error).toBe("Device error");
			expect(state.isLoading).toBe(false);
		});
	});

	describe("setGeneratorFrequency", () => {
		it("should update generator frequency", () => {
			useAudioStore.getState().setGeneratorFrequency(2000);

			expect(useAudioStore.getState().generatorFrequency).toBe(2000);
		});
	});

	describe("setGeneratorAmplitude", () => {
		it("should update generator amplitude", () => {
			useAudioStore.getState().setGeneratorAmplitude(0.8);

			expect(useAudioStore.getState().generatorAmplitude).toBe(0.8);
		});
	});

	describe("setGeneratorType", () => {
		it("should update generator type", () => {
			useAudioStore.getState().setGeneratorType("pink_noise");

			expect(useAudioStore.getState().generatorType).toBe("pink_noise");
		});
	});

	describe("setSelectedInputDevice", () => {
		it("should update selected input device", () => {
			useAudioStore.getState().setSelectedInputDevice("input:NewDevice");

			expect(useAudioStore.getState().selectedInputDevice).toBe("input:NewDevice");
		});

		it("should allow setting to null", () => {
			useAudioStore.getState().setSelectedInputDevice("input:Test");
			useAudioStore.getState().setSelectedInputDevice(null);

			expect(useAudioStore.getState().selectedInputDevice).toBeNull();
		});
	});

	describe("setSelectedOutputDevice", () => {
		it("should update selected output device", () => {
			useAudioStore.getState().setSelectedOutputDevice("output:NewDevice");

			expect(useAudioStore.getState().selectedOutputDevice).toBe("output:NewDevice");
		});
	});

	describe("clearError", () => {
		it("should clear error state", () => {
			useAudioStore.setState({ error: "Some error" });

			useAudioStore.getState().clearError();

			expect(useAudioStore.getState().error).toBeNull();
		});
	});

	describe("startStream", () => {
		it("should call API and update state on success", async () => {
			mockInvoke
				.mockResolvedValueOnce(undefined) // start_audio_stream
				.mockResolvedValueOnce({ is_running: true, sample_rate: 48000, buffer_size: 512 }); // get_audio_state

			await useAudioStore.getState().startStream();

			const state = useAudioStore.getState();
			expect(state.audioState?.is_running).toBe(true);
			expect(state.isLoading).toBe(false);
		});

		it("should handle errors", async () => {
			mockInvoke.mockRejectedValueOnce(new Error("Stream error"));

			await useAudioStore.getState().startStream();

			const state = useAudioStore.getState();
			expect(state.error).toBe("Stream error");
			expect(state.isLoading).toBe(false);
		});
	});

	describe("stopStream", () => {
		it("should call API and update state", async () => {
			mockInvoke
				.mockResolvedValueOnce(undefined) // stop_audio_stream
				.mockResolvedValueOnce({ is_running: false }); // get_audio_state

			await useAudioStore.getState().stopStream();

			expect(mockInvoke).toHaveBeenCalledWith("stop_audio_stream");
		});
	});

	describe("startGenerator", () => {
		it("should call API with current generator settings", async () => {
			useAudioStore.setState({
				generatorType: "sine",
				generatorFrequency: 1000,
				generatorAmplitude: 0.5,
			});
			mockInvoke
				.mockResolvedValueOnce(undefined) // start_generator
				.mockResolvedValueOnce({ generator_active: true }); // get_audio_state

			await useAudioStore.getState().startGenerator();

			expect(mockInvoke).toHaveBeenCalledWith("start_generator", {
				signal_type: "sine",
				frequency: 1000,
				amplitude: 0.5,
			});
		});
	});

	describe("stopGenerator", () => {
		it("should call stop_generator command", async () => {
			mockInvoke
				.mockResolvedValueOnce(undefined) // stop_generator
				.mockResolvedValueOnce({ generator_active: false }); // get_audio_state

			await useAudioStore.getState().stopGenerator();

			expect(mockInvoke).toHaveBeenCalledWith("stop_generator");
		});
	});

	describe("fetchSpectrum", () => {
		it("should update spectrum data silently", async () => {
			const mockSpectrum = new Array(1025).fill(-60);
			mockInvoke.mockResolvedValueOnce(mockSpectrum);

			await useAudioStore.getState().fetchSpectrum();

			expect(useAudioStore.getState().spectrumData).toEqual(mockSpectrum);
		});

		it("should not set error on failure (silent fail)", async () => {
			mockInvoke.mockRejectedValueOnce(new Error("Fetch error"));

			await useAudioStore.getState().fetchSpectrum();

			// Should not set error for spectrum polling
			expect(useAudioStore.getState().error).toBeNull();
		});
	});

	describe("resetAveraging", () => {
		it("should call reset_averaging command", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await useAudioStore.getState().resetAveraging();

			expect(mockInvoke).toHaveBeenCalledWith("reset_averaging");
		});
	});

	describe("setFftSize", () => {
		it("should call API and update local state", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await useAudioStore.getState().setFftSize(4096);

			expect(mockInvoke).toHaveBeenCalledWith("set_fft_size", { size: 4096 });
			expect(useAudioStore.getState().fftSize).toBe(4096);
		});
	});
});
