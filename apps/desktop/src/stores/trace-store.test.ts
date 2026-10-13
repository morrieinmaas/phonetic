import { beforeEach, describe, expect, it } from "bun:test";
import type { SessionSettings, TransferFunctionData } from "../types/audio";
import { useTraceStore } from "./trace-store";

const makeTransferFunctionData = (): TransferFunctionData => ({
	magnitude_db: [0, -3, -6],
	phase_deg: [0, -45, -90],
	coherence: [1, 0.95, 0.9],
	group_delay_ms: [0, 0.5, 1],
	frequencies: [100, 1000, 10000],
});

describe("session serialisation", () => {
	const settings: SessionSettings = {
		sample_rate: 48000,
		fft_size: 4096,
		multi_window: true,
		smoothing: "octave6",
		delay_samples: 480,
		max_averages: 16,
	};

	beforeEach(() => {
		useTraceStore.getState().clearAllTraces();
	});

	it("round-trips traces through a session", () => {
		const store = useTraceStore.getState();
		store.clearAllTraces();
		const id = store.captureTrace(makeTransferFunctionData(), "Left");

		const session = useTraceStore.getState().toSession("Main PA", settings);
		expect(session.version).toBe(1);
		expect(session.name).toBe("Main PA");
		expect(session.traces).toHaveLength(1);

		useTraceStore.getState().clearAllTraces();
		expect(useTraceStore.getState().traces).toHaveLength(0);

		useTraceStore.getState().fromSession(session);
		const restored = useTraceStore.getState().traces;
		expect(restored).toHaveLength(1);
		expect(restored[0]?.name).toBe("Left");
		expect(restored[0]?.id).toBe(id);
	});

	it("replaces rather than merges on load", () => {
		const store = useTraceStore.getState();
		store.clearAllTraces();
		store.captureTrace(makeTransferFunctionData(), "Stale");

		const empty = { version: 1, created_at: "", name: "Empty", settings, traces: [] };
		useTraceStore.getState().fromSession(empty);
		expect(useTraceStore.getState().traces).toHaveLength(0);
	});
});
