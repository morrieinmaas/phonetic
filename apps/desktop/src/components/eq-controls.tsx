import { useEqStore } from "../stores/eq-store";
import type { FilterType } from "../types/audio";

const FILTER_TYPES: { value: FilterType; label: string }[] = [
	{ value: "peaking", label: "Peaking" },
	{ value: "low_shelf", label: "Low Shelf" },
	{ value: "high_shelf", label: "High Shelf" },
	{ value: "high_pass", label: "High Pass" },
	{ value: "low_pass", label: "Low Pass" },
];

/** Filter-bank controls: add/remove/edit biquad filters, and bypass the whole bank. */
export const EqControls = () => {
	const { filters, bypassed, addFilter, updateFilter, removeFilter, toggleFilter, toggleBypass } =
		useEqStore();

	return (
		<div data-testid="eq-controls" className="flex flex-col gap-2 text-xs">
			<div className="flex items-center justify-between">
				<h3 className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
					Virtual EQ
				</h3>
				<div className="flex items-center gap-2">
					<button
						type="button"
						data-testid="eq-bypass"
						onClick={toggleBypass}
						disabled={filters.length === 0}
						className={`px-2 py-1 rounded border transition-colors disabled:opacity-40 disabled:cursor-not-allowed ${
							bypassed
								? "border-border text-muted-foreground hover:bg-accent"
								: "border-primary bg-primary/10 text-primary"
						}`}
					>
						{bypassed ? "Bypassed" : "Active"}
					</button>
					<button
						type="button"
						data-testid="eq-add-filter"
						onClick={addFilter}
						className="px-2 py-1 rounded border border-border hover:bg-accent"
					>
						+ Filter
					</button>
				</div>
			</div>

			{filters.length > 0 && (
				<div className="flex flex-col gap-1">
					{filters.map((f) => (
						<div
							key={f.id}
							data-testid="eq-filter-row"
							className="flex items-center gap-2 rounded border border-border/50 px-2 py-1"
						>
							<input
								type="checkbox"
								checked={f.enabled}
								onChange={() => toggleFilter(f.id)}
								title={f.enabled ? "Disable filter" : "Enable filter"}
							/>
							<select
								value={f.filter_type}
								onChange={(e: React.ChangeEvent<HTMLSelectElement>) =>
									updateFilter(f.id, { filter_type: e.target.value as FilterType })
								}
								className="px-1 py-0.5 rounded border border-border bg-background"
							>
								{FILTER_TYPES.map((t) => (
									<option key={t.value} value={t.value}>
										{t.label}
									</option>
								))}
							</select>
							<input
								type="number"
								value={f.frequency_hz}
								min={20}
								max={20000}
								onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
									updateFilter(f.id, { frequency_hz: Number(e.target.value) })
								}
								className="w-16 px-1 py-0.5 rounded border border-border bg-background"
								aria-label="Frequency in hertz"
							/>
							<span className="text-muted-foreground">Hz</span>
							<input
								type="number"
								value={f.gain_db}
								step={0.5}
								onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
									updateFilter(f.id, { gain_db: Number(e.target.value) })
								}
								className="w-14 px-1 py-0.5 rounded border border-border bg-background"
								aria-label="Gain in decibels"
							/>
							<span className="text-muted-foreground">dB</span>
							<input
								type="number"
								value={f.q}
								min={0.01}
								step={0.1}
								onChange={(e: React.ChangeEvent<HTMLInputElement>) =>
									updateFilter(f.id, { q: Number(e.target.value) })
								}
								className="w-14 px-1 py-0.5 rounded border border-border bg-background"
								aria-label="Q factor"
							/>
							<span className="text-muted-foreground">Q</span>
							<button
								type="button"
								data-testid="eq-remove"
								onClick={() => removeFilter(f.id)}
								className="ml-auto text-muted-foreground hover:text-destructive"
								aria-label="Remove filter"
							>
								×
							</button>
						</div>
					))}
				</div>
			)}
		</div>
	);
};
