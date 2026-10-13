import { describe, expect, it } from "bun:test";
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

/**
 * Guards against building UI that nothing renders.
 *
 * This project has shipped a component that was never mounted three times.
 * TraceControls existed, was tested, and was unreachable until someone opened
 * the app and looked for it. RoomCriteriaPanel was heading the same way.
 *
 * Tests on a component prove the component works. They say nothing about
 * whether an operator can ever open it. This test asserts every component is
 * imported by something other than itself and its own test file, which is the
 * cheapest thing that would have caught all three.
 */

const SRC = join(import.meta.dir);
const COMPONENTS = join(SRC, "components");

const sourceFiles = (dir: string): string[] =>
	readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
		const path = join(dir, entry.name);
		if (entry.isDirectory()) {
			return sourceFiles(path);
		}
		return entry.name.endsWith(".tsx") || entry.name.endsWith(".ts") ? [path] : [];
	});

describe("every component is reachable", () => {
	const allSources = sourceFiles(SRC);

	const components = readdirSync(COMPONENTS, { withFileTypes: true })
		.filter((e) => e.isFile() && e.name.endsWith(".tsx") && !e.name.includes(".test."))
		.map((e) => e.name.replace(/\.tsx$/, ""));

	it("finds components to check", () => {
		expect(components.length).toBeGreaterThan(5);
	});

	for (const name of components) {
		it(`${name} is imported by something else`, () => {
			const importers = allSources.filter((file) => {
				if (file.endsWith(`components/${name}.tsx`)) return false;
				if (file.includes(`components/${name}.test.`)) return false;
				const text = readFileSync(file, "utf8");
				return text.includes(`./${name}"`) || text.includes(`components/${name}"`);
			});

			expect(
				importers.length,
				`${name}.tsx is imported by nothing — it cannot be opened from the UI`,
			).toBeGreaterThan(0);
		});
	}
});
