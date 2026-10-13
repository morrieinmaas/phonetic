import { describe, expect, it } from "bun:test";
import type { Diagnosis } from "../types/audio";
import { selectTopDiagnosis } from "./diagnostics-banner";

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
