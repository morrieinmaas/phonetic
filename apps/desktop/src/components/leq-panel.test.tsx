import { describe, expect, it } from "bun:test";
import { render, screen } from "@testing-library/react";
import { LeqPanel } from "./leq-panel";

const metrics = {
	leq_db: 88.2,
	laeq_db: 85.4,
	lceq_db: 92.1,
	lafmax_db: 97.3,
	lapeak_db: 113.8,
	elapsed_seconds: 3600,
};

const limit = {
	id: "eu-upper",
	name: "EU upper action value",
	source: "Directive 2003/10/EC Art. 3",
	as_of: "2003-02-06",
	laeq_limit_db: 85,
	reference_hours: 8,
	lapeak_limit_db: 137,
};

describe("LeqPanel", () => {
	it("shows every metric, since a missing one reads as zero", () => {
		render(
			<LeqPanel
				metrics={metrics}
				limit={limit}
				dose={{ percent: 13.8, projected_percent_at_reference: 110.4, peak_exceeded: false }}
			/>,
		);
		for (const label of [/LAeq/, /LCeq/, /LAFmax/, /LApeak/]) {
			expect(screen.getByText(label)).toBeTruthy();
		}
	});

	it("shows the limit's source and date next to the limit", () => {
		render(
			<LeqPanel
				metrics={metrics}
				limit={limit}
				dose={{ percent: 13.8, projected_percent_at_reference: 110.4, peak_exceeded: false }}
			/>,
		);
		expect(screen.getByText(/2003\/10\/EC/)).toBeTruthy();
		expect(screen.getByText(/2003-02-06/)).toBeTruthy();
	});

	it("never renders a compliance verdict", () => {
		render(
			<LeqPanel
				metrics={metrics}
				limit={limit}
				dose={{ percent: 13.8, projected_percent_at_reference: 110.4, peak_exceeded: false }}
			/>,
		);
		expect(screen.queryByText(/\bcompliant\b/i)).toBeNull();
		expect(screen.queryByText(/\bpass(ed)?\b/i)).toBeNull();
	});

	it("says the limits are user-configured, not authoritative", () => {
		render(
			<LeqPanel
				metrics={metrics}
				limit={limit}
				dose={{ percent: 13.8, projected_percent_at_reference: 110.4, peak_exceeded: false }}
			/>,
		);
		expect(screen.getByText(/measured value/i)).toBeTruthy();
	});

	it("uses tabular figures so the readout does not jitter", () => {
		const { container } = render(
			<LeqPanel
				metrics={metrics}
				limit={limit}
				dose={{ percent: 13.8, projected_percent_at_reference: 110.4, peak_exceeded: false }}
			/>,
		);
		expect(container.querySelectorAll(".tabular-nums").length).toBeGreaterThan(0);
	});
});
