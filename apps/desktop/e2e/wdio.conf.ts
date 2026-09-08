// Side-effect type import: `@wdio/local-runner`'s own .d.ts is what pulls in
// `@wdio/types`'s ambient `WebdriverIO.Config` augmentation. Nothing else in
// this file references the runner directly.
import type {} from "@wdio/local-runner";
import type { TauriCapabilities } from "@wdio/tauri-service";

// tauri-driver supports Windows and Linux only — macOS has no WKWebView driver
// tool. `driverProvider: 'embedded'` runs a WebDriver server inside the app
// itself (via tauri-plugin-wdio-webdriver), which is the supported macOS route.
const appBinaryPath =
	"./src-tauri/target/release/bundle/macos/Phonetic.app/Contents/MacOS/Phonetic";

// @wdio/tauri-service requires `browserName: "tauri"` plus a
// `tauri:options.application` path on the capability itself — an empty
// capability object is accepted by older versions of the service but not by
// the one this project depends on. Declared as its own constant (rather than
// inline in `config`) so TypeScript's excess-property check runs against
// `TauriCapabilities`, which knows about the `tauri:options` vendor key,
// instead of against `WebdriverIO.Config`'s generic capabilities type, which
// doesn't.
const capabilities: TauriCapabilities[] = [
	{
		browserName: "tauri",
		"tauri:options": {
			application: appBinaryPath,
		},
	},
];

export const config: WebdriverIO.Config = {
	runner: "local",
	specs: ["./specs/**/*.e2e.ts"],
	maxInstances: 1,
	logLevel: "warn",
	bail: 0,
	waitforTimeout: 10_000,
	connectionRetryTimeout: 90_000,
	connectionRetryCount: 3,
	services: [
		[
			"@wdio/tauri-service",
			{
				appBinaryPath,
				driverProvider: "embedded",
				// There is no per-test `browser.getLogs()` command in the current
				// WebdriverIO API — this is the replacement: forward frontend and
				// backend console output to the test runner's own output instead.
				captureFrontendLogs: true,
				captureBackendLogs: true,
			},
		],
	],
	capabilities,
	framework: "mocha",
	reporters: ["spec"],
	mochaOpts: {
		ui: "bdd",
		timeout: 120_000,
	},
};
