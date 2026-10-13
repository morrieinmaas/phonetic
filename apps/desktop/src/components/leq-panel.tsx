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
	/** Plain-language meaning, for the engineer who does not read LAFmax fluently. */
	hint: string;
	value: string;
}

/** One numeric readout. `tabular-nums` keeps the digits from jittering in width as they change. */
const MetricStat = ({ label, hint, value }: MetricStatProps) => (
	<div className="flex flex-col gap-0.5 rounded-md border border-border px-3 py-2" title={hint}>
		<span className="text-xs text-muted-foreground">
			<span className="font-medium text-foreground">{label}</span> {hint}
		</span>
		<span className="tabular-nums font-mono text-lg font-semibold text-foreground">{value}</span>
	</div>
);

/** "38 min", "2 h 10 min": when the dose reaches 100 % if the current level holds. */
export const formatTimeToLimit = (dose: DoseResult, referenceHours: number): string | null => {
	if (dose.projected_percent_at_reference <= 0) return null;
	const minutes = Math.round((referenceHours * 60 * 100) / dose.projected_percent_at_reference);
	if (minutes >= referenceHours * 60) return null;
	const hours = Math.floor(minutes / 60);
	return hours > 0 ? `${hours} h ${minutes % 60} min` : `${Math.max(1, minutes)} min`;
};

const doseTone = (percent: number) =>
	percent >= 100 ? "bg-red-500" : percent >= 50 ? "bg-amber-500" : "bg-emerald-500";

/**
 * LEQ / noise-dose display.
 *
 * Copy rules: never a compliance verdict ("compliant", "pass"); the limits are
 * user-configured presets, not authoritative thresholds; every number is a
 * measured value, not a certified determination; no IEC 61672-1 conformance
 * claim, the A/C weighting is nominal (see `dsp::leq`).
 *
 * Layout: fills the view and scrolls inside itself. It is not a card of its
 * own: the view container already is one, and a nested card overflowed it.
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
	const timeToLimit = dose && limit ? formatTimeToLimit(dose, limit.reference_hours) : null;

	return (
		<section data-testid="leq-panel" className="flex h-full min-h-0 flex-col gap-4">
			<div className="flex flex-shrink-0 items-center justify-between gap-2">
				<div className="flex items-baseline gap-3">
					<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
						Noise Dose
					</h2>
					{metrics && (
						<span className="text-xs text-muted-foreground">
							Elapsed{" "}
							<span className="font-mono tabular-nums text-foreground">
								{formatElapsed(metrics.elapsed_seconds)}
							</span>
						</span>
					)}
				</div>
				{canToggleLogging && (
					<button
						type="button"
						data-testid="leq-logging-toggle"
						onClick={isLogging ? onStopLogging : onStartLogging}
						className={`rounded-md border px-2.5 py-1 text-xs transition-colors ${
							isLogging
								? "border-red-500 bg-red-500 text-white hover:bg-red-600"
								: "border-border hover:bg-muted"
						}`}
					>
						{isLogging ? "Stop logging" : "Log to CSV…"}
					</button>
				)}
			</div>

			{isLogging && loggingPath && (
				<p
					className="-mt-2 flex-shrink-0 truncate text-xs text-muted-foreground"
					title={loggingPath}
				>
					Logging every minute to {loggingPath}
				</p>
			)}

			<div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto">
				{metrics ? (
					<div className="grid grid-cols-2 gap-2 lg:grid-cols-4">
						<MetricStat label="LAeq" hint="A-weighted average" value={formatDb(metrics.laeq_db)} />
						<MetricStat label="LCeq" hint="C-weighted average" value={formatDb(metrics.lceq_db)} />
						<MetricStat
							label="LAFmax"
							hint="Loudest moment, fast"
							value={formatDb(metrics.lafmax_db)}
						/>
						<MetricStat label="LApeak" hint="Highest peak" value={formatDb(metrics.lapeak_db)} />
					</div>
				) : (
					<p className="text-sm text-muted-foreground">
						Start the stream to measure. Readings are only meaningful with a calibrated microphone
						(Settings).
					</p>
				)}

				<div className="flex flex-col gap-3 rounded-md border border-border p-3">
					<div className="flex flex-wrap items-center gap-2">
						<label htmlFor="leq-limit-select" className="text-xs text-muted-foreground">
							Compare with
						</label>
						{onSelectLimit ? (
							<select
								id="leq-limit-select"
								value={limit?.id ?? ""}
								onChange={(event) => onSelectLimit(event.target.value)}
								disabled={availableLimits.length === 0}
								className="min-w-0 max-w-full truncate rounded border border-border bg-card px-1.5 py-1 text-xs text-foreground"
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
						) : (
							<span className="text-xs text-foreground">{limit?.name ?? "No limit selected"}</span>
						)}
						{limit && (
							<span className="text-xs text-muted-foreground">
								<span className="font-mono tabular-nums text-foreground">
									{limit.laeq_limit_db} dB
								</span>{" "}
								over {limit.reference_hours} h
							</span>
						)}
					</div>

					{dose && (
						<div className="flex flex-col gap-2">
							<div className="flex flex-wrap items-baseline gap-x-4 gap-y-1">
								<span className="text-sm text-muted-foreground">
									Dose{" "}
									<span className="font-mono text-2xl font-semibold tabular-nums text-foreground">
										{dose.percent.toFixed(1)}%
									</span>
								</span>
								<span className="text-xs text-muted-foreground">
									{timeToLimit
										? `At the current level the limit is reached after ${timeToLimit}.`
										: `At the current level: ${dose.projected_percent_at_reference.toFixed(0)}% by the end of ${limit?.reference_hours ?? 8} h.`}
								</span>
							</div>
							<div
								className="h-2 overflow-hidden rounded-full bg-muted"
								role="meter"
								aria-label="Dose of the reference period"
								aria-valuemin={0}
								aria-valuemax={100}
								aria-valuenow={Math.min(100, dose.percent)}
							>
								<div
									className={`h-full ${doseTone(dose.percent)}`}
									style={{ width: `${Math.min(100, dose.percent)}%` }}
								/>
							</div>
							<p className="text-xs text-muted-foreground">
								C-weighted peak ceiling:{" "}
								<span
									className={dose.peak_exceeded ? "font-medium text-red-500" : "text-foreground"}
								>
									{dose.peak_exceeded ? "exceeded" : "not exceeded"}
								</span>
							</p>
						</div>
					)}

					{limit && (
						<details className="text-xs text-muted-foreground">
							<summary className="cursor-pointer select-none hover:text-foreground">
								Where this limit comes from
							</summary>
							<p className="mt-1 max-w-prose leading-relaxed">
								{limit.source} (as of {limit.as_of}).
							</p>
						</details>
					)}
				</div>

				<p className="max-w-prose text-xs text-muted-foreground">
					Limits are editable presets, not legal thresholds. Each reading is a measured value from
					this app's calibration, not a certified compliance determination. A and C weighting are
					nominal, not verified to Class 1.
				</p>
			</div>
		</section>
	);
};
