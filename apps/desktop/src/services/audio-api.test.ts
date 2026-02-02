import { beforeEach, describe, expect, it, mock } from "bun:test";

// Mock Tauri's invoke function
const mockInvoke = mock(() => Promise.resolve({}));

mock.module("@tauri-apps/api/core", () => ({
	invoke: mockInvoke,
}));

// Import after mocking
import { audioApi } from "./audio-api";

describe("audioApi", () => {
	beforeEach(() => {
		mockInvoke.mockClear();
	});

	describe("listDevices", () => {
		it("should call list_audio_devices command", async () => {
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
				outputs: [],
				default_input: "input:Test",
				default_output: null,
			};
			mockInvoke.mockResolvedValueOnce(mockDevices);

			const result = await audioApi.listDevices();

			expect(mockInvoke).toHaveBeenCalledWith("list_audio_devices");
			expect(result).toEqual(mockDevices);
		});
	});

	describe("getState", () => {
		it("should call get_audio_state command", async () => {
			const mockState = {
				is_running: true,
				sample_rate: 48000,
				buffer_size: 512,
				input_device: "Test Input",
				output_device: "Test Output",
				generator_active: false,
				generator_type: null,
			};
			mockInvoke.mockResolvedValueOnce(mockState);

			const result = await audioApi.getState();

			expect(mockInvoke).toHaveBeenCalledWith("get_audio_state");
			expect(result).toEqual(mockState);
		});
	});

	describe("startStream", () => {
		it("should call start_audio_stream with device IDs", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await audioApi.startStream("input:Test", "output:Test");

			expect(mockInvoke).toHaveBeenCalledWith("start_audio_stream", {
				input_device_id: "input:Test",
				output_device_id: "output:Test",
			});
		});

		it("should pass null for undefined device IDs", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await audioApi.startStream();

			expect(mockInvoke).toHaveBeenCalledWith("start_audio_stream", {
				input_device_id: null,
				output_device_id: null,
			});
		});
	});

	describe("startGenerator", () => {
		it("should call start_generator with valid parameters", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await audioApi.startGenerator("sine", 1000, 0.5);

			expect(mockInvoke).toHaveBeenCalledWith("start_generator", {
				signal_type: "sine",
				frequency: 1000,
				amplitude: 0.5,
			});
		});

		it("should throw error for frequency below minimum", () => {
			expect(() => audioApi.startGenerator("sine", 10, 0.5)).toThrow(
				"Frequency must be between 20 and 20000 Hz",
			);
		});

		it("should throw error for frequency above maximum", () => {
			expect(() => audioApi.startGenerator("sine", 25000, 0.5)).toThrow(
				"Frequency must be between 20 and 20000 Hz",
			);
		});

		it("should throw error for amplitude below minimum", () => {
			expect(() => audioApi.startGenerator("sine", 1000, -0.5)).toThrow(
				"Amplitude must be between 0 and 1",
			);
		});

		it("should throw error for amplitude above maximum", () => {
			expect(() => audioApi.startGenerator("sine", 1000, 1.5)).toThrow(
				"Amplitude must be between 0 and 1",
			);
		});

		it("should throw error for NaN frequency", () => {
			expect(() => audioApi.startGenerator("sine", Number.NaN, 0.5)).toThrow(
				"Frequency must be between 20 and 20000 Hz",
			);
		});

		it("should throw error for Infinity amplitude", () => {
			expect(() => audioApi.startGenerator("sine", 1000, Number.POSITIVE_INFINITY)).toThrow(
				"Amplitude must be between 0 and 1",
			);
		});
	});

	describe("setFftSize", () => {
		it("should call set_fft_size with valid size", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await audioApi.setFftSize(2048);

			expect(mockInvoke).toHaveBeenCalledWith("set_fft_size", { size: 2048 });
		});

		it("should throw error for invalid FFT size", () => {
			expect(() => audioApi.setFftSize(1000)).toThrow(
				"FFT size must be one of: 512, 1024, 2048, 4096, 8192, 16384",
			);
		});

		it("should accept all valid FFT sizes", async () => {
			for (const size of [512, 1024, 2048, 4096, 8192, 16384]) {
				mockInvoke.mockResolvedValueOnce(undefined);
				await audioApi.setFftSize(size);
				expect(mockInvoke).toHaveBeenCalledWith("set_fft_size", { size });
			}
		});
	});

	describe("getSpectrum", () => {
		it("should call get_spectrum_data command", async () => {
			const mockSpectrum = new Array(1025).fill(-60);
			mockInvoke.mockResolvedValueOnce(mockSpectrum);

			const result = await audioApi.getSpectrum();

			expect(mockInvoke).toHaveBeenCalledWith("get_spectrum_data");
			expect(result).toEqual(mockSpectrum);
		});
	});

	describe("getTransferFunction", () => {
		it("should call get_transfer_function_data command", async () => {
			const mockData = {
				magnitude_db: [],
				phase_deg: [],
				coherence: [],
				group_delay_ms: [],
				frequencies: [],
			};
			mockInvoke.mockResolvedValueOnce(mockData);

			const result = await audioApi.getTransferFunction();

			expect(mockInvoke).toHaveBeenCalledWith("get_transfer_function_data");
			expect(result).toEqual(mockData);
		});
	});

	describe("stopStream", () => {
		it("should call stop_audio_stream command", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await audioApi.stopStream();

			expect(mockInvoke).toHaveBeenCalledWith("stop_audio_stream");
		});
	});

	describe("stopGenerator", () => {
		it("should call stop_generator command", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await audioApi.stopGenerator();

			expect(mockInvoke).toHaveBeenCalledWith("stop_generator");
		});
	});

	describe("resetAveraging", () => {
		it("should call reset_averaging command", async () => {
			mockInvoke.mockResolvedValueOnce(undefined);

			await audioApi.resetAveraging();

			expect(mockInvoke).toHaveBeenCalledWith("reset_averaging");
		});
	});
});
