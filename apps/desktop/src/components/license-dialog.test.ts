import { describe, expect, it } from "bun:test";
import { shouldBlockUsage, trialMessage } from "./license-dialog";

describe("licence gating", () => {
	it("lets a licensed user through", () => {
		expect(shouldBlockUsage({ state: "licensed", key_suffix: "MNOP" })).toBe(false);
	});

	it("lets a trial user through", () => {
		expect(shouldBlockUsage({ state: "trial", days_remaining: 3 })).toBe(false);
	});

	it("blocks an expired trial", () => {
		expect(shouldBlockUsage({ state: "expired" })).toBe(true);
	});
});

describe("trialMessage", () => {
	it("says nothing to a licensed user", () => {
		expect(trialMessage({ state: "licensed", key_suffix: "MNOP" })).toBeNull();
	});

	it("says nothing to an expired user", () => {
		expect(trialMessage({ state: "expired" })).toBeNull();
	});

	it("counts the days down", () => {
		expect(trialMessage({ state: "trial", days_remaining: 3 })).toContain("3 days");
	});

	it("uses the singular on the last day", () => {
		expect(trialMessage({ state: "trial", days_remaining: 1 })).toContain("1 day left");
		expect(trialMessage({ state: "trial", days_remaining: 1 })).not.toContain("days");
	});
});
