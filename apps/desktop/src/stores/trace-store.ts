import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
	Session,
	SessionSettings,
	StoredTrace,
	TraceSlot,
	TransferFunctionData,
} from "../types/audio";

/** Predefined colors for traces */
const TRACE_COLORS = [
	"oklch(0.7 0.18 30)", // Orange-red (Trace A)
	"oklch(0.7 0.18 160)", // Teal (Trace B)
	"oklch(0.7 0.18 300)", // Magenta (Trace C)
	"oklch(0.7 0.18 60)", // Yellow (Trace D)
	"oklch(0.65 0.15 220)", // Light blue
	"oklch(0.65 0.15 90)", // Lime
	"oklch(0.65 0.15 330)", // Pink
	"oklch(0.65 0.15 180)", // Cyan
];

interface TraceStore {
	/** All stored traces */
	traces: StoredTrace[];

	/** Quick-access trace slots (A, B, C, D) */
	slots: Record<TraceSlot, string | null>;

	/** Maximum number of traces to store */
	maxTraces: number;

	/** Capture current measurement as a new trace */
	captureTrace: (data: TransferFunctionData, name?: string) => string;

	/** Store trace to a specific slot */
	storeToSlot: (slot: TraceSlot, traceId: string | null) => void;

	/** Capture directly to a slot */
	captureToSlot: (slot: TraceSlot, data: TransferFunctionData) => void;

	/** Toggle trace visibility */
	toggleTraceVisibility: (traceId: string) => void;

	/** Rename a trace */
	renameTrace: (traceId: string, name: string) => void;

	/** Delete a trace */
	deleteTrace: (traceId: string) => void;

	/** Clear all traces */
	clearAllTraces: () => void;

	/** Get trace by ID */
	getTrace: (traceId: string) => StoredTrace | undefined;

	/** Get trace in a slot */
	getSlotTrace: (slot: TraceSlot) => StoredTrace | undefined;

	/** Get all visible traces */
	getVisibleTraces: () => StoredTrace[];

	/** Snapshot the store as a session document. */
	toSession: (name: string, settings: SessionSettings) => Session;

	/** Replace the store's contents with a loaded session. */
	fromSession: (session: Session) => void;
}

export const useTraceStore = create<TraceStore>()(
	persist(
		(set, get) => ({
			traces: [],
			slots: { A: null, B: null, C: null, D: null },
			maxTraces: 20,

			captureTrace: (data, name) => {
				const { traces, maxTraces } = get();
				const id = `trace-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
				const colorIndex = traces.length % TRACE_COLORS.length;

				const newTrace: StoredTrace = {
					id,
					name: name ?? `Trace ${traces.length + 1}`,
					color: TRACE_COLORS[colorIndex] ?? TRACE_COLORS[0] ?? "oklch(0.7 0.15 0)",
					timestamp: Date.now(),
					data: structuredClone(data),
					visible: true,
				};

				// Remove oldest traces if we exceed max
				const updatedTraces =
					traces.length >= maxTraces ? [...traces.slice(1), newTrace] : [...traces, newTrace];

				set({ traces: updatedTraces });
				return id;
			},

			storeToSlot: (slot, traceId) => {
				set((state) => ({
					slots: { ...state.slots, [slot]: traceId },
				}));
			},

			captureToSlot: (slot, data) => {
				const { traces, maxTraces, slots } = get();

				// If slot already has a trace, update it instead of creating new
				const existingTraceId = slots[slot];
				if (existingTraceId) {
					const existingTrace = traces.find((t) => t.id === existingTraceId);
					if (existingTrace) {
						set({
							traces: traces.map((t) =>
								t.id === existingTraceId
									? { ...t, data: structuredClone(data), timestamp: Date.now() }
									: t,
							),
						});
						return;
					}
				}

				// Create new trace for this slot
				const id = `trace-${slot}-${Date.now()}`;
				const slotColors: Record<TraceSlot, string> = {
					A: TRACE_COLORS[0] ?? "oklch(0.7 0.18 30)",
					B: TRACE_COLORS[1] ?? "oklch(0.7 0.18 160)",
					C: TRACE_COLORS[2] ?? "oklch(0.7 0.18 300)",
					D: TRACE_COLORS[3] ?? "oklch(0.7 0.18 60)",
				};

				const newTrace: StoredTrace = {
					id,
					name: `Trace ${slot}`,
					color: slotColors[slot],
					timestamp: Date.now(),
					data: structuredClone(data),
					visible: true,
				};

				const updatedTraces =
					traces.length >= maxTraces ? [...traces.slice(1), newTrace] : [...traces, newTrace];

				set({
					traces: updatedTraces,
					slots: { ...slots, [slot]: id },
				});
			},

			toggleTraceVisibility: (traceId) => {
				set((state) => ({
					traces: state.traces.map((t) => (t.id === traceId ? { ...t, visible: !t.visible } : t)),
				}));
			},

			renameTrace: (traceId, name) => {
				set((state) => ({
					traces: state.traces.map((t) => (t.id === traceId ? { ...t, name } : t)),
				}));
			},

			deleteTrace: (traceId) => {
				set((state) => ({
					traces: state.traces.filter((t) => t.id !== traceId),
					slots: Object.fromEntries(
						Object.entries(state.slots).map(([slot, id]) => [slot, id === traceId ? null : id]),
					) as Record<TraceSlot, string | null>,
				}));
			},

			clearAllTraces: () => {
				set({ traces: [], slots: { A: null, B: null, C: null, D: null } });
			},

			getTrace: (traceId) => {
				return get().traces.find((t) => t.id === traceId);
			},

			getSlotTrace: (slot) => {
				const traceId = get().slots[slot];
				return traceId ? get().traces.find((t) => t.id === traceId) : undefined;
			},

			getVisibleTraces: () => {
				return get().traces.filter((t) => t.visible);
			},

			toSession: (name, settings) => ({
				version: 1,
				created_at: new Date().toISOString(),
				name,
				settings,
				traces: get().traces,
			}),

			fromSession: (session) => {
				set({
					traces: session.traces,
					slots: { A: null, B: null, C: null, D: null },
				});
			},
		}),
		{
			name: "phonetic-traces",
			partialize: (state) => ({
				traces: state.traces,
				slots: state.slots,
			}),
		},
	),
);
