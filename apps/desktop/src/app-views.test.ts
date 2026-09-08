import { describe, expect, it } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

/**
 * Guards that every secondary view is reachable from the tab row.
 *
 * A view can be present in the view map, fully implemented and tested, and
 * still be unreachable because nothing renders a button for it — the same
 * failure as a component nobody mounts. The tab row was also a hand-written
 * segmented control with `overflow-hidden`, so a fifth tab that did not fit
 * was silently clipped rather than wrapped.
 */

const source = readFileSync(join(import.meta.dir, "app.tsx"), "utf8");

const views = (source.match(/type SecondaryView = ([^;]+);/)?.[1] ?? "")
	.split("|")
	.map((v) => v.trim().replace(/"/g, ""))
	.filter(Boolean);

describe("secondary views", () => {
	it("finds the view union", () => {
		expect(views.length).toBeGreaterThanOrEqual(5);
	});

	it("lists every view in SECONDARY_VIEWS", () => {
		const listed = source.match(/const SECONDARY_VIEWS[\s\S]*?\n\];/)?.[0] ?? "";
		for (const view of views) {
			expect(listed.includes(`"${view}"`), `${view} missing from SECONDARY_VIEWS`).toBe(true);
		}
	});

	it("gives every view a tab label, so none can ship unlabelled", () => {
		const labels =
			source.match(/SECONDARY_VIEW_LABELS: Record<SecondaryView, string> = \{[^}]+\}/s)?.[0] ?? "";
		for (const view of views) {
			expect(labels.includes(`${view}:`), `${view} has no tab label`).toBe(true);
		}
	});

	it("renders the tab row from the list rather than hand-written buttons", () => {
		// Five copies of the same button markup is how a tab gets forgotten.
		expect(source.includes("SECONDARY_VIEWS.map(")).toBe(true);
	});

	it("lets the tab row wrap instead of clipping tabs that do not fit", () => {
		// Strip JSX comments first: this file explains that overflow-hidden was
		// removed, and matching that prose would make the test pass or fail on
		// a comment rather than on the markup.
		const code = source.replace(/\{\/\*[\s\S]*?\*\/\}/g, "");
		const row = code.match(/View:[\s\S]{0,900}?SECONDARY_VIEWS\.map/)?.[0] ?? "";
		expect(row.length, "could not locate the tab row").toBeGreaterThan(0);
		expect(row.includes("flex-wrap"), "the tab row must wrap").toBe(true);
		expect(row.includes("overflow-hidden"), "overflow-hidden clips tabs").toBe(false);
	});
});
