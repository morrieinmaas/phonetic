/**
 * Registers happy-dom globals for `bun test` component tests (render/screen from
 * @testing-library/react need `document`/`window` on globalThis before they're imported).
 *
 * Bun's own docs point at the separate `@happy-dom/global-registrator` package for this, but
 * this project only carries plain `happy-dom` as a devDependency. Rather than add a new
 * dependency, this inlines that package's registration algorithm (own properties of a
 * `GlobalWindow` instance, skipped when the global already holds the identical value — e.g.
 * `Object`/`Promise`/`Array`, which `GlobalWindow` itself sets to the host's own globals) using
 * only what's already installed.
 */
import { GlobalWindow, PropertySymbol } from "happy-dom";

const IGNORE_LIST = new Set(["constructor", "undefined", "NaN", "global", "globalThis"]);

const window = new GlobalWindow({ console: globalThis.console });
const globalRecord = globalThis as unknown as Record<string | symbol, unknown>;
const windowRecord = window as unknown as Record<string | symbol, unknown>;

const propertyDescriptors = Object.getOwnPropertyDescriptors(window);
for (const key of Object.keys(propertyDescriptors)) {
	if (IGNORE_LIST.has(key)) continue;

	const windowDescriptor = propertyDescriptors[key];
	if (!windowDescriptor) continue;
	const globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, key);
	if (globalDescriptor?.value !== undefined && globalDescriptor.value === windowDescriptor.value) {
		continue;
	}

	// The window's self-referencing properties (window.window, window.self, ...) should point
	// at the global object once registered, not at the detached window instance.
	if (windowDescriptor.value === window) {
		windowRecord[key] = globalThis;
		windowDescriptor.value = globalThis;
	}

	Object.defineProperty(globalThis, key, { ...windowDescriptor, configurable: true });
}

for (const key of Object.getOwnPropertySymbols(window)) {
	const descriptor = Object.getOwnPropertyDescriptor(window, key);
	if (!descriptor) continue;
	if (descriptor.value === window) {
		windowRecord[key] = globalThis;
		descriptor.value = globalThis;
	}
	Object.defineProperty(globalThis, key, { ...descriptor, configurable: true });
}

(globalRecord.document as Record<symbol, unknown>)[PropertySymbol.defaultView] = globalThis;
