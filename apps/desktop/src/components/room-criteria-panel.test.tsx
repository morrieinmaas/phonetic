import { describe, expect, it } from "bun:test";
import { render, screen } from "@testing-library/react";
import { RoomCriteriaPanel } from "./room-criteria-panel";

const result = {
	family: "nc" as const,
	rating: 30,
	exceeded_band_hz: 63,
	band_levels_db: [57, 48, 41, 35, 31, 29, 28, 27],
	curve_db: [57, 48, 41, 35, 31, 29, 28, 27],
};

describe("RoomCriteriaPanel", () => {
	it("names the rating and the band that determined it", () => {
		render(<RoomCriteriaPanel result={result} onFamilyChange={() => {}} />);
		expect(screen.getByText(/NC-30/)).toBeTruthy();
		expect(screen.getByText(/63\s*Hz/)).toBeTruthy();
	});

	it("shows every octave band, not a truncated subset", () => {
		render(<RoomCriteriaPanel result={result} onFamilyChange={() => {}} />);
		for (const hz of [63, 125, 250, 500, 1000, 2000, 4000, 8000]) {
			expect(screen.getAllByText(new RegExp(String(hz))).length).toBeGreaterThan(0);
		}
	});

	it("carries the not-certified-compliance disclaimer", () => {
		render(<RoomCriteriaPanel result={result} onFamilyChange={() => {}} />);
		expect(screen.getByText(/measured value/i)).toBeTruthy();
	});
});
