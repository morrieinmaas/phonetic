import { describe, expect, it, mock } from "bun:test";
import { fireEvent, render, screen } from "@testing-library/react";
import type { MeasurementPair } from "../types/audio";
import { MeasurementList, measurementProblem, nextFreeChannel } from "./measurement-list";

const pair = (
	id: string,
	channel: number,
	extra: Partial<MeasurementPair> = {},
): MeasurementPair => ({
	id,
	name: id,
	measurement_channel: channel,
	reference: { source: "generator" },
	delay_samples: 0,
	calibration_offset_db: 0,
	enabled: true,
	...extra,
});

const renderList = (
	pairs: MeasurementPair[],
	channelCount = 4,
	onChange: (routing: { pairs: MeasurementPair[] }) => void = () => {},
) =>
	render(
		<MeasurementList
			routing={{ pairs }}
			channelCount={channelCount}
			onChange={onChange}
			viewedPairId={null}
			onView={() => {}}
		/>,
	);

describe("MeasurementList", () => {
	it("explains itself when there are no measurements", () => {
		renderList([]);
		expect(screen.getByText(/no measurements yet/i)).toBeTruthy();
	});

	it("adds a measurement on the first unused input", () => {
		const onChange = mock((_: unknown) => {});
		renderList([pair("Mic 1", 0)], 4, onChange);
		fireEvent.click(screen.getByTestId("add-measurement"));
		const next = (onChange.mock.calls[0]?.[0] as { pairs: MeasurementPair[] }).pairs;
		expect(next).toHaveLength(2);
		expect(next[1]?.measurement_channel).toBe(1);
		expect(next[1]?.name).toBe("Mic 2");
	});

	it("removes a measurement", () => {
		const onChange = mock((_: unknown) => {});
		renderList([pair("a", 0), pair("b", 1)], 4, onChange);
		fireEvent.click(screen.getByLabelText("Remove a"));
		expect(
			(onChange.mock.calls[0]?.[0] as { pairs: MeasurementPair[] }).pairs.map((p) => p.id),
		).toEqual(["b"]);
	});

	it("refuses a measurement referenced to its own input", () => {
		const onChange = mock(() => {});
		renderList([pair("a", 0)], 4, onChange);
		fireEvent.change(screen.getByLabelText(/reference for a/i), { target: { value: "input:0" } });
		expect(onChange).not.toHaveBeenCalled();
		expect(screen.getByRole("alert").textContent).toMatch(/own input/i);
	});

	it("tells a one-channel user how to measure more microphones", () => {
		renderList([], 1);
		expect(screen.getByText(/raise/i)).toBeTruthy();
	});
});

describe("measurement helpers", () => {
	it("picks the first free input, falling back to the first", () => {
		expect(nextFreeChannel([pair("a", 0), pair("b", 2)], 4)).toBe(1);
		expect(nextFreeChannel([pair("a", 0)], 1)).toBe(0);
	});

	it("names an input that is not open", () => {
		expect(measurementProblem(pair("a", 3), 2)).toMatch(/Input 4/);
		expect(measurementProblem(pair("a", 1), 2)).toBeNull();
	});
});
