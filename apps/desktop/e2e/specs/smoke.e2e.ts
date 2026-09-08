import { $, expect } from "@wdio/globals";

describe("Phonetic launches", () => {
	it("shows the status bar", async () => {
		await expect($('[data-testid="status-bar"]')).toBeDisplayed();
	});

	it("populates the input device list", async () => {
		const select = $('[data-testid="input-device-select"]');
		await expect(select).toBeDisplayed();
		const options = await select.$$("option");
		expect(options.length).toBeGreaterThan(0);
	});

	it("opens and closes the settings dialog", async () => {
		await $('[data-testid="settings-open"]').click();
		await expect($('[data-testid="settings-dialog"]')).toBeDisplayed();
		// settings-dialog.tsx has no global Escape-key handler — only the
		// backdrop click, the header's X button, and the footer's "Done"
		// button close it. The X button carries a stable aria-label, so use
		// that rather than a CSS class.
		await $('[aria-label="Close settings"]').click();
		await expect($('[data-testid="settings-dialog"]')).not.toBeDisplayed();
	});

	// The brief's "no console errors on startup" check used
	// `browser.getLogs("browser")`, a legacy JSON-Wire command. It has been
	// removed from WebdriverIO's command set entirely (not just deprecated —
	// there is no `getLogs` anywhere in the installed `webdriverio` package),
	// so there is no direct API to assert against here. Console/log capture
	// for a Tauri session is a service-level concern instead — see
	// `captureFrontendLogs`/`captureBackendLogs` in wdio.conf.ts's tauri
	// service options, which forward logs to the test runner's own output
	// rather than exposing them as a queryable command.
});
