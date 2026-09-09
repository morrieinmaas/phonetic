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

const rcResult = {
	level: 35,
	quality: "low_frequency" as const,
	band_levels_db: [70, 60, 45, 40, 37, 35, 34, 33, 32],
	bins_per_band: [0, 1, 5, 5, 5, 5, 5, 5, 5],
	reference_db: [50, 46, 42, 40, 38, 36, 35, 30, 25],
	under_resolved: true,
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

	it("does not show RC's 16 Hz or 31.5 Hz bands in NC mode", () => {
		render(<RoomCriteriaPanel result={result} onFamilyChange={() => {}} />);
		expect(screen.queryByText("16")).toBeNull();
		expect(screen.queryByText("31.5")).toBeNull();
	});

	it("renders the RC level and quality letter with the letter's meaning spelled out", () => {
		render(
			<RoomCriteriaPanel family="rc" result={null} rcResult={rcResult} onFamilyChange={() => {}} />,
		);
		expect(screen.getByText(/RC-35 \(LF\)/)).toBeTruthy();
		expect(screen.getByText(/low-frequency rumble/i)).toBeTruthy();
	});

	it("shows the under-resolved warning and mentions FFT size when under_resolved is true", () => {
		render(
			<RoomCriteriaPanel family="rc" result={null} rcResult={rcResult} onFamilyChange={() => {}} />,
		);
		expect(screen.getByText(/under-resolved/i)).toBeTruthy();
		expect(screen.getByText(/FFT size/i)).toBeTruthy();
	});

	it("shows no under-resolved warning when under_resolved is false", () => {
		render(
			<RoomCriteriaPanel
				family="rc"
				result={null}
				rcResult={{ ...rcResult, under_resolved: false }}
				onFamilyChange={() => {}}
			/>,
		);
		expect(screen.queryByText(/under-resolved/i)).toBeNull();
		expect(screen.queryByTestId("rc-under-resolved-warning")).toBeNull();
	});

	it("shows RC's 16 Hz and 31.5 Hz bands in RC mode", () => {
		render(
			<RoomCriteriaPanel family="rc" result={null} rcResult={rcResult} onFamilyChange={() => {}} />,
		);
		expect(screen.getByText("16")).toBeTruthy();
		expect(screen.getByText("31.5")).toBeTruthy();
	});
});
