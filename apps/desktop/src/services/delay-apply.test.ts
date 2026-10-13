import { describe, expect, it, mock } from "bun:test";

describe("applyMeasuredDelay", () => {
	it("passes the found delay through to the backend", async () => {
		const setDelay = mock(() => Promise.resolve());
		const { applyMeasuredDelay } = await import("./delay-apply");
		await applyMeasuredDelay({ delay_samples: 480, is_valid: true }, setDelay);
		expect(setDelay).toHaveBeenCalledWith(480);
	});

	it("refuses to apply a delay the finder did not trust", async () => {
		const setDelay = mock(() => Promise.resolve());
		const { applyMeasuredDelay } = await import("./delay-apply");
		await applyMeasuredDelay({ delay_samples: 480, is_valid: false }, setDelay);
		expect(setDelay).not.toHaveBeenCalled();
	});

	it("refuses to apply a negative delay even if marked valid", async () => {
		const setDelay = mock(() => Promise.resolve());
		const { applyMeasuredDelay } = await import("./delay-apply");
		await applyMeasuredDelay({ delay_samples: -1, is_valid: true }, setDelay);
		expect(setDelay).not.toHaveBeenCalled();
	});
});
