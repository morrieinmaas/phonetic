import { describe, expect, it } from "bun:test";
import { isSecondaryView } from "./app";

/**
 * `RoomCriteriaPanel` was fully built and tested but never mounted anywhere in the app,
 * so an operator had no way to reach it — this is the failure mode this project has
 * shipped before (`TraceControls` went unmounted the same way). Rendering the full `App`
 * component in this test suite isn't practical: it always mounts `WebGLSpectrum` (a raw
 * WebGL canvas via three.js) and `SpectrumAnalyzer` (uPlot, which needs real canvas 2D
 * context and imports a CSS file), none of which happy-dom's `bun test` environment
 * supports. Instead this locks in the localStorage restore contract for the "room" view:
 * a previously-stored value must still work, and an unrecognised stored value must not
 * wedge the app into a state with nothing to render.
 */
describe("isSecondaryView", () => {
	it("accepts the room criteria view", () => {
		expect(isSecondaryView("room")).toBe(true);
	});

	it("accepts the two pre-existing views", () => {
		expect(isSecondaryView("transfer")).toBe(true);
		expect(isSecondaryView("spectrogram")).toBe(true);
	});

	it("rejects a value that was never a valid view", () => {
		expect(isSecondaryView("waterfall")).toBe(false);
	});

	it("rejects an empty string and null", () => {
		expect(isSecondaryView("")).toBe(false);
		expect(isSecondaryView(null)).toBe(false);
	});
});
