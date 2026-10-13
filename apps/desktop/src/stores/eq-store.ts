import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { BiquadFilter } from "../types/audio";

/** A biquad filter as held by the UI: the wire shape plus a client-side id. */
export interface EqFilter extends BiquadFilter {
	id: string;
}

const DEFAULT_FILTER: BiquadFilter = {
	filter_type: "peaking",
	frequency_hz: 1000,
	gain_db: 0,
	q: 1,
	enabled: true,
};

interface EqStore {
	/** The filter bank, applied in series. */
	filters: EqFilter[];

	/** When true, the predicted-curve overlay is suppressed without discarding the filters. */
	bypassed: boolean;

	/** Add a filter with sensible defaults. */
	addFilter: () => void;

	/** Patch a filter's fields by id, leaving the others untouched. */
	updateFilter: (id: string, patch: Partial<BiquadFilter>) => void;

	/** Remove a filter by id. */
	removeFilter: (id: string) => void;

	/** Toggle a single filter's enabled flag. */
	toggleFilter: (id: string) => void;

	/** Remove every filter. */
	clearFilters: () => void;

	/** Toggle bypass for the whole bank. */
	toggleBypass: () => void;
}

export const useEqStore = create<EqStore>()(
	persist(
		(set) => ({
			filters: [],
			bypassed: false,

			addFilter: () => {
				const filter: EqFilter = { id: crypto.randomUUID(), ...DEFAULT_FILTER };
				set((state) => ({ filters: [...state.filters, filter] }));
			},

			updateFilter: (id, patch) => {
				set((state) => ({
					filters: state.filters.map((f) => (f.id === id ? { ...f, ...patch } : f)),
				}));
			},

			removeFilter: (id) => {
				set((state) => ({ filters: state.filters.filter((f) => f.id !== id) }));
			},

			toggleFilter: (id) => {
				set((state) => ({
					filters: state.filters.map((f) => (f.id === id ? { ...f, enabled: !f.enabled } : f)),
				}));
			},

			clearFilters: () => {
				set({ filters: [] });
			},

			toggleBypass: () => {
				set((state) => ({ bypassed: !state.bypassed }));
			},
		}),
		{
			name: "phonetic-eq",
		},
	),
);
