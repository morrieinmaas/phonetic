import { describe, expect, it } from "bun:test";
import type { Diagnosis } from "../types/audio";
import { rearmDismissed, reconcileDiagnoses, selectTopDiagnosis } from "./diagnostics-banner";

const make = (code: string, severity: Diagnosis["severity"]): Diagnosis => ({
	code,
	severity,
	message: code,
	action: null,
	delay_samples: null,
});

describe("selectTopDiagnosis", () => {
	it("returns null when nothing is wrong", () => {
		expect(selectTopDiagnosis([], new Set())).toBeNull();
	});

	it("prefers an error over a warning", () => {
		const top = selectTopDiagnosis([make("a", "warning"), make("b", "error")], new Set());
		expect(top?.code).toBe("b");
	});

	it("skips a diagnosis the operator dismissed", () => {
		const top = selectTopDiagnosis([make("a", "error"), make("b", "warning")], new Set(["a"]));
		expect(top?.code).toBe("b");
	});

	it("returns null when everything is dismissed", () => {
		expect(selectTopDiagnosis([make("a", "error")], new Set(["a"]))).toBeNull();
	});
});

describe("reconcileDiagnoses", () => {
	it("keeps a diagnosis for one clean poll, then drops it", () => {
		const once = reconcileDiagnoses([], [make("low_coherence", "warning")]);
		const firstClean = reconcileDiagnoses(once, []);
		expect(firstClean.map((s) => s.diagnosis.code)).toEqual(["low_coherence"]);
		expect(reconcileDiagnoses(firstClean, [])).toEqual([]);
	});

	it("resets the grace period when the problem is reported again", () => {
		const fading = reconcileDiagnoses(reconcileDiagnoses([], [make("a", "warning")]), []);
		const back = reconcileDiagnoses(fading, [make("a", "warning")]);
		expect(back).toEqual([{ diagnosis: make("a", "warning"), misses: 0 }]);
	});
});

describe("rearmDismissed", () => {
	it("forgets a dismissal once the problem is gone", () => {
		expect(rearmDismissed(new Set(["a"]), [])).toEqual(new Set());
	});

	it("keeps a dismissal while the problem is still shown", () => {
		const dismissed = new Set(["a"]);
		expect(rearmDismissed(dismissed, [{ diagnosis: make("a", "warning"), misses: 0 }])).toBe(
			dismissed,
		);
	});
});
