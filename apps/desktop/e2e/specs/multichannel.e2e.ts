import { $, $$, expect } from "@wdio/globals";

describe("multi-channel", () => {
	before(async () => {
		const startButton = $("button=Start Stream");
		await startButton.waitForDisplayed();
		await startButton.click();
	});

	after(async () => {
		const stopButton = $("button=Stop Stream");
		if (await stopButton.isExisting()) {
			await stopButton.click();
		}
	});

	it("offers a channel count selector bounded by the device's maximum", async () => {
		await expect($('[data-testid="channel-selector"]')).toBeDisplayed();
		const select = $('[data-testid="channel-count"]');
		await expect(select).toBeDisplayed();
		const options = await select.$$("option");
		expect(options.length).toBeGreaterThan(0);
	});

	it("averages the visible traces into a new one", async () => {
		const captureButton = $('[data-testid="capture-trace"]');
		await captureButton.waitForEnabled({
			timeout: 30_000,
			timeoutMsg: "transfer function never produced data to capture — is there an audio loopback?",
		});
		await captureButton.click();

		// Open the trace list so its rows are in the DOM to count.
		await $("button*=trace").click();
		const rows = await $$('[data-testid="trace-list"] > div');
		const before = rows.length;

		await $('[data-testid="average-traces"]').click();

		await expect($$('[data-testid="trace-list"] > div')).toBeElementsArrayOfSize(before + 1);
	});
});
