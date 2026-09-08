import { beforeEach, describe, expect, it } from "bun:test";
import { useEqStore } from "./eq-store";

describe("eq store", () => {
	beforeEach(() => useEqStore.getState().clearFilters());

	it("starts empty", () => {
		expect(useEqStore.getState().filters).toHaveLength(0);
	});

	it("adds a filter with sensible defaults", () => {
		useEqStore.getState().addFilter();
		const [f] = useEqStore.getState().filters;
		expect(f?.filter_type).toBe("peaking");
		expect(f?.frequency_hz).toBe(1000);
		expect(f?.gain_db).toBe(0);
		expect(f?.q).toBe(1);
		expect(f?.enabled).toBe(true);
		expect(typeof f?.id).toBe("string");
	});

	it("assigns each filter a distinct id", () => {
		useEqStore.getState().addFilter();
		useEqStore.getState().addFilter();
		const [first, second] = useEqStore.getState().filters;
		expect(first?.id).toBeDefined();
		expect(second?.id).toBeDefined();
		expect(first?.id).not.toBe(second?.id);
	});

	it("updates a filter by id without touching the others", () => {
		useEqStore.getState().addFilter();
		useEqStore.getState().addFilter();
		const [first, second] = useEqStore.getState().filters;
		expect(first).toBeDefined();
		expect(second).toBeDefined();
		if (!first || !second) throw new Error("expected two filters");

		useEqStore.getState().updateFilter(first.id, { gain_db: -6 });

		const after = useEqStore.getState().filters;
		expect(after.find((f) => f.id === first.id)?.gain_db).toBe(-6);
		expect(after.find((f) => f.id === second.id)?.gain_db).toBe(0);
	});

	it("removes a filter", () => {
		useEqStore.getState().addFilter();
		const [f] = useEqStore.getState().filters;
		expect(f).toBeDefined();
		if (!f) throw new Error("expected a filter");

		useEqStore.getState().removeFilter(f.id);
		expect(useEqStore.getState().filters).toHaveLength(0);
	});

	it("toggles a filter's enabled flag", () => {
		useEqStore.getState().addFilter();
		const [f] = useEqStore.getState().filters;
		expect(f).toBeDefined();
		if (!f) throw new Error("expected a filter");

		useEqStore.getState().toggleFilter(f.id);
		expect(useEqStore.getState().filters.find((x) => x.id === f.id)?.enabled).toBe(false);

		useEqStore.getState().toggleFilter(f.id);
		expect(useEqStore.getState().filters.find((x) => x.id === f.id)?.enabled).toBe(true);
	});

	it("toggles bypass without discarding filters", () => {
		useEqStore.getState().addFilter();
		useEqStore.getState().toggleBypass();
		expect(useEqStore.getState().bypassed).toBe(true);
		expect(useEqStore.getState().filters).toHaveLength(1);

		useEqStore.getState().toggleBypass();
		expect(useEqStore.getState().bypassed).toBe(false);
	});

	it("clears every filter", () => {
		useEqStore.getState().addFilter();
		useEqStore.getState().addFilter();
		useEqStore.getState().clearFilters();
		expect(useEqStore.getState().filters).toHaveLength(0);
	});
});
