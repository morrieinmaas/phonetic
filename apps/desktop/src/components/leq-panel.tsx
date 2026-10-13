import type { DoseResult, LeqMetrics, NoiseLimit } from "../types/audio";

/**
 * The subset of `LeqMetrics` this panel renders. LCpeak is tracked by the
 * meter and used server-side by `dose()`'s peak comparison, but the display
 * spec calls for LAeq/LCeq/LAFmax/LApeak only, so it is deliberately not
 * part of this prop's required shape.
 */
export type LeqPanelMetrics = Pick<
	LeqMetrics,
	"leq_db" | "laeq_db" | "lceq_db" | "lafmax_db" | "lapeak_db" | "elapsed_seconds"
>;

/** The subset of `NoiseLimit` this panel renders. */
export type LeqPanelLimit = Pick<
	NoiseLimit,
	"id" | "name" | "source" | "as_of" | "laeq_limit_db" | "reference_hours"
>;

interface LeqPanelProps {
	metrics: LeqPanelMetrics | null;
	limit: LeqPanelLimit | null;
	dose: DoseResult | null;
	/** Every limit the operator can pick from. Omitted (or empty) hides the selector. */
	availableLimits?: LeqPanelLimit[];
	onSelectLimit?: (id: string) => void;
	isLogging?: boolean;
	loggingPath?: string | null;
	onStartLogging?: () => void;
	onStopLogging?: () => void;
}

const formatDb = (value: number): string => `${value.toFixed(1)} dB`;

const formatElapsed = (seconds: number): string => {
	const total = Math.max(0, Math.floor(seconds));
	const hours = Math.floor(total / 3600);
	const minutes = Math.floor((total % 3600) / 60);
	const secs = total % 60;
	const pad = (n: number) => n.toString().padStart(2, "0");
	return `${hours}:${pad(minutes)}:${pad(secs)}`;
};

interface MetricStatProps {
	label: string;
	value: string;
}

/** One numeric readout. `tabular-nums` keeps the digits from jittering in width as they change. */
const MetricStat = ({ label, value }: MetricStatProps) => (
	<div className="flex flex-col gap-0.5 rounded-md border border-border bg-card px-3 py-2">
		<span className="text-[11px] uppercase tracking-wider text-muted-foreground">{label}</span>
		<span className="tabular-nums font-mono text-lg font-semibold text-foreground">{value}</span>
	</div>
);

/**
 * LEQ / noise-dose display.
 *
 * Copy rules (see the spec and plan) drive every line of text here:
 * - Never a compliance verdict. No "compliant", no "pass" -- whether a
 *   measurement is acceptable is the operator's judgement against their own
 *   site rules, not this app's.
 * - The limits are user-configured presets, not authoritative thresholds,
 *   and every number here is a measured value, not a certified compliance
 *   determination.
 * - No IEC 61672-1 conformance claim: the A/C-weighting filters are nominal
 *   to the standard's curves, not verified against its Class 1 tolerance
 *   table (see `dsp::leq`).
 *
 * Layout: this panel declares its own minimum height and flex-column
 * layout; the content scrolls *inside* it via the `min-h-0` container below
 * -- never both a pixel minimum and `min-h-0` on the same element, or their
 * minimums fight and the page grows instead of the panel scrolling.
 */
export const LeqPanel = ({
	metrics,
	limit,
	dose,
	availableLimits = [],
	onSelectLimit,
	isLogging = false,
	loggingPath = null,
	onStartLogging,
	onStopLogging,
}: LeqPanelProps) => {
	const canToggleLogging = Boolean(onStartLogging && onStopLogging);

	return (
		<section
			data-testid="leq-panel"
			className="glass flex min-h-[420px] flex-1 flex-col gap-3 rounded-lg p-4"
		>
			<div className="flex flex-shrink-0 items-center justify-between gap-2">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Noise Dose
				</h2>
				{canToggleLogging && (
					<button
						type="button"
						data-testid="leq-logging-toggle"
						onClick={isLogging ? onStopLogging : onStartLogging}
						className={`rounded border border-border px-2 py-1 text-xs transition-colors ${
							isLogging ? "bg-red-500 text-white hover:bg-red-600" : "hover:bg-muted"
						}`}
					>
						{isLogging ? "Stop logging" : "Start logging…"}
					</button>
				)}
			</div>

			{isLogging && loggingPath && (
				<p className="flex-shrink-0 truncate text-[11px] text-muted-foreground" title={loggingPath}>
					Logging to {loggingPath}
				</p>
			)}

			<div className="min-h-0 flex-1 overflow-y-auto">
				{metrics ? (
					<div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
						<MetricStat label="LAeq" value={formatDb(metrics.laeq_db)} />
						<MetricStat label="LCeq" value={formatDb(metrics.lceq_db)} />
						<MetricStat label="LAFmax" value={formatDb(metrics.lafmax_db)} />
						<MetricStat label="LApeak" value={formatDb(metrics.lapeak_db)} />
					</div>
				) : (
					<p className="text-sm text-muted-foreground">Waiting for a measurement…</p>
				)}

				{metrics && (
					<p className="mt-2 text-xs text-muted-foreground">
						Elapsed{" "}
						<span className="tabular-nums font-mono">{formatElapsed(metrics.elapsed_seconds)}</span>
					</p>
				)}

				<div className="mt-4 flex flex-col gap-2 border-t border-border pt-3">
					{onSelectLimit && (
						<div className="flex items-center gap-2">
							<label htmlFor="leq-limit-select" className="text-xs text-muted-foreground">
								Limit
							</label>
							<select
								id="leq-limit-select"
								value={limit?.id ?? ""}
								onChange={(event) => onSelectLimit(event.target.value)}
								disabled={availableLimits.length === 0}
								className="rounded border border-border bg-card px-1 py-0.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
							>
								{limit && !availableLimits.some((candidate) => candidate.id === limit.id) && (
									<option value={limit.id}>{limit.name}</option>
								)}
								{availableLimits.map((candidate) => (
									<option key={candidate.id} value={candidate.id}>
										{candidate.name}
									</option>
								))}
							</select>
						</div>
					)}

					{limit ? (
						<p className="text-xs text-muted-foreground">
							<span className="font-medium text-foreground">{limit.name}</span> —{" "}
							<span className="tabular-nums font-mono">{limit.laeq_limit_db}</span> dB over{" "}
							<span className="tabular-nums font-mono">{limit.reference_hours}</span> h. Source:{" "}
							{limit.source} (as of {limit.as_of}).
						</p>
					) : (
						<p className="text-xs text-muted-foreground">No limit selected.</p>
					)}

					{dose && (
						<p className="text-sm text-foreground">
							Dose:{" "}
							<span className="tabular-nums font-mono font-semibold">
								{dose.percent.toFixed(1)}%
							</span>{" "}
							of the reference period ({dose.projected_percent_at_reference.toFixed(0)}% projected
							at the current level). C-weighted peak over ceiling:{" "}
							<span className="font-medium">{dose.peak_exceeded ? "yes" : "no"}</span>.
						</p>
					)}

					<p className="text-[11px] text-muted-foreground">
						Limits shown are user-configured presets, not authoritative thresholds. Every reading
						here is a measured value from this app's own calibration, not a certified compliance
						determination — the operator judges the reading against the limit. The A- and
						C-weighting filters above are nominal, not verified against a Class 1 tolerance table.
					</p>
				</div>
			</div>
		</section>
	);
};
