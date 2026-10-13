import { describe, expect, it, mock } from "bun:test";
import { fireEvent, render, screen } from "@testing-library/react";
import { ChannelMatrix } from "./channel-matrix";

const routing = {
	pairs: [
		{
			id: "a",
			name: "Main L",
			measurement_channel: 0,
			reference: { source: "generator" as const },
			delay_samples: 0,
			calibration_offset_db: 0,
			enabled: true,
		},
		{
			id: "b",
			name: "Sub",
			measurement_channel: 1,
			reference: { source: "input" as const, channel: 7 },
			delay_samples: 512,
			calibration_offset_db: -3,
			enabled: true,
		},
	],
};

describe("ChannelMatrix", () => {
	it("renders a row for every available channel", () => {
		render(<ChannelMatrix routing={routing} channelCount={32} onChange={() => {}} />);
		expect(screen.getAllByRole("row").length).toBeGreaterThanOrEqual(32);
	});

	it("shows what each pair references", () => {
		render(<ChannelMatrix routing={routing} channelCount={32} onChange={() => {}} />);
		expect(screen.getByText(/generator/i)).toBeTruthy();
		expect(screen.getByText(/input 8/i)).toBeTruthy(); // channel 7, shown one-based
	});

	it("shows channels one-based, because that is what the hardware is labelled", () => {
		render(<ChannelMatrix routing={routing} channelCount={32} onChange={() => {}} />);
		expect(screen.getByText("1")).toBeTruthy();
		expect(screen.getByText("32")).toBeTruthy();
		expect(screen.queryByText("0")).toBeNull();
	});

	it("refuses a pair referenced to its own channel", () => {
		const onChange = mock(() => {});
		render(<ChannelMatrix routing={routing} channelCount={32} onChange={onChange} />);
		// Point pair "a" (measuring channel 0) at input 0 as its reference.
		fireEvent.change(screen.getByLabelText(/reference for Main L/i), {
			target: { value: "input:0" },
		});
		expect(onChange).not.toHaveBeenCalled();
		expect(screen.getByText(/cannot reference itself/i)).toBeTruthy();
	});

	it("scrolls inside its own container rather than growing the page", () => {
		const { container } = render(
			<ChannelMatrix routing={routing} channelCount={32} onChange={() => {}} />,
		);
		const scroller = container.querySelector("[data-matrix-scroll]");
		expect(scroller).toBeTruthy();
		expect(scroller?.className).toContain("overflow-y-auto");
		expect(scroller?.className).toContain("min-h-0");
	});
});

// Controller ruling B8: multi-window mode and the pair matrix are mutually exclusive, and that
// exclusion must be visible — a frozen trace that still looks live is worse than either
// alternative on its own.
describe("ChannelMatrix while multi-window is active (ruling B8)", () => {
	it("shows a paused message instead of the grid", () => {
		render(
			<ChannelMatrix
				routing={routing}
				channelCount={32}
				onChange={() => {}}
				isMultiWindow={true}
			/>,
		);
		expect(screen.getByText(/paused.*multi-window active/i)).toBeTruthy();
		expect(screen.queryByRole("table")).toBeNull();
	});

	it("does not offer the reference controls that would otherwise edit routing", () => {
		render(
			<ChannelMatrix
				routing={routing}
				channelCount={32}
				onChange={() => {}}
				isMultiWindow={true}
			/>,
		);
		expect(screen.queryByLabelText(/reference for Main L/i)).toBeNull();
	});
});
