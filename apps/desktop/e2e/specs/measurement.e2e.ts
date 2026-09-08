import { $, $$, expect } from "@wdio/globals";

describe("measurement workflow", () => {
	before(async () => {
		// generator-controls.tsx disables the generator toggle until the
		// audio stream is running (app.tsx passes `disabled={!isRunning}`),
		// and trace-controls.tsx's capture button stays disabled until a
		// non-empty transfer function reading exists — both downstream of
		// starting the stream here. "Start Stream" only renders once the
		// device list has loaded.
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

	it("changes FFT size without crashing", async () => {
		// settings-dialog.tsx renders FFT size as a button grid, not a
		// native <select> — there's no `.selectByAttribute()` target here.
		await $('[data-testid="settings-open"]').click();
		await $('[data-testid="fft-size-8192"]').click();
		await expect($('[data-testid="fft-size-current"]')).toHaveText("8192");
		// The dialog only closes via the backdrop, the X button, or "Done" —
		// clicking the dialog panel itself does nothing.
		await $('[aria-label="Close settings"]').click();
	});

	it("starts and stops the signal generator", async () => {
		const toggle = $('[data-testid="generator-toggle"]');
		await toggle.click();
		await expect(toggle).toHaveText(expect.stringContaining("Stop"));
		await toggle.click();
		await expect(toggle).toHaveText(expect.stringContaining("Start"));
	});

	it("captures a trace and lists it", async () => {
		// Capturing needs currentData.frequencies to be non-empty, which
		// needs the generator running and a real (or looped-back) audio
		// signal reaching the input device. This step requires actual audio
		// I/O and will not pass on a machine with no hardware/loopback
		// wired up.
		const toggle = $('[data-testid="generator-toggle"]');
		if ((await toggle.getText()).includes("Start")) {
			await toggle.click();
		}

		const captureButton = $('[data-testid="capture-trace"]');
		await captureButton.waitForEnabled({
			timeout: 30_000,
			timeoutMsg: "transfer function never produced data to capture — is there an audio loopback?",
		});
		await captureButton.click();

		// trace-controls.tsx only renders the "Stored Traces" dropdown once
		// traces.length > 0, and its rows are plain <div>s, not <li>s.
		await $("button*=1 trace").click();
		const rows = await $$('[data-testid="trace-list"] > div');
		expect(rows.length).toBe(1);
	});
});
