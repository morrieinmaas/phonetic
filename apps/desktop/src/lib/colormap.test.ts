import { describe, expect, it } from "bun:test";
import { magnitudeToRgb } from "./colormap";

describe("magnitudeToRgb", () => {
	it("maps the floor to the darkest colour", () => {
		const [r, g, b] = magnitudeToRgb(-100, -100, 0);
		expect(r + g + b).toBeLessThan(90);
	});

	it("maps the ceiling to the brightest colour", () => {
		const [r, g, b] = magnitudeToRgb(0, -100, 0);
		expect(r + g + b).toBeGreaterThan(500);
	});

	it("clamps below the floor and above the ceiling", () => {
		expect(magnitudeToRgb(-999, -100, 0)).toEqual(magnitudeToRgb(-100, -100, 0));
		expect(magnitudeToRgb(999, -100, 0)).toEqual(magnitudeToRgb(0, -100, 0));
	});

	it("is monotonic in brightness", () => {
		const brightness = (db: number) => {
			const [r, g, b] = magnitudeToRgb(db, -100, 0);
			return r + g + b;
		};
		for (let db = -100; db < 0; db += 10) {
			expect(brightness(db + 10)).toBeGreaterThanOrEqual(brightness(db));
		}
	});

	it("always returns channels in range", () => {
		for (let db = -140; db <= 20; db += 5) {
			for (const c of magnitudeToRgb(db, -100, 0)) {
				expect(c).toBeGreaterThanOrEqual(0);
				expect(c).toBeLessThanOrEqual(255);
			}
		}
	});

	it("handles a degenerate range without dividing by zero", () => {
		for (const c of magnitudeToRgb(-50, -50, -50)) {
			expect(Number.isFinite(c)).toBe(true);
		}
	});
});
