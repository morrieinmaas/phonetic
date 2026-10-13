import { useState } from "react";
import type { MeasurementPair, ReferenceSource, Routing } from "../types/audio";

interface MeasurementListProps {
	routing: Routing;
	/** Input channels the stream opens; set in the sidebar. */
	channelCount: number;
	/** Called with the whole updated routing; the caller applies and may reject it. */
	onChange: (routing: Routing) => void;
	/** Which measurement the Transfer Function view shows; null is the sidebar channel. */
	viewedPairId: string | null;
	onView: (pairId: string | null) => void;
	/**
	 * Multi-window mode shares one processor across every measurement, so per-measurement
	 * results would all be the same trace. The list says so instead of looking live.
	 */
	isMultiWindow?: boolean;
}

const MAX_PAIRS = 32;

const referenceValue = (reference: ReferenceSource): string =>
	reference.source === "generator" ? "generator" : `input:${reference.channel}`;

const parseReference = (value: string): ReferenceSource =>
	value === "generator"
		? { source: "generator" }
		: { source: "input", channel: Number(value.slice("input:".length)) };

/** The first input no measurement uses yet, so a new row starts somewhere useful. */
export const nextFreeChannel = (pairs: MeasurementPair[], channelCount: number): number => {
	const used = new Set(pairs.map((pair) => pair.measurement_channel));
	return Array.from({ length: channelCount }, (_, i) => i).find((i) => !used.has(i)) ?? 0;
};

export const newMeasurement = (pairs: MeasurementPair[], channelCount: number): MeasurementPair => {
	const channel = nextFreeChannel(pairs, channelCount);
	return {
		id: crypto.randomUUID(),
		name: `Mic ${channel + 1}`,
		measurement_channel: channel,
		reference: { source: "generator" },
		delay_samples: 0,
		calibration_offset_db: 0,
		enabled: true,
	};
};

/** Why a measurement cannot work as configured, in words, or null when it is fine. */
export const measurementProblem = (pair: MeasurementPair, channelCount: number): string | null => {
	if (pair.measurement_channel >= channelCount) {
		return `Input ${pair.measurement_channel + 1} is not open. Raise Input channels in the sidebar.`;
	}
	if (pair.reference.source === "input" && pair.reference.channel >= channelCount) {
		return `Reference input ${pair.reference.channel + 1} is not open.`;
	}
	if (pair.reference.source === "input" && pair.reference.channel === pair.measurement_channel) {
		return "A measurement cannot use its own input as the reference.";
	}
	return null;
};

const fieldClass =
	"rounded border border-border bg-card px-1.5 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring";

/**
 * The measurements Phonetic runs side by side: each one compares a microphone input with a
 * reference, so several mic positions can be measured from one sweep or one stretch of pink
 * noise. Rows are measurements, the thing an engineer names and thinks in, not channels.
 */
export const MeasurementList = ({
	routing,
	channelCount,
	onChange,
	viewedPairId,
	onView,
	isMultiWindow = false,
}: MeasurementListProps) => {
	const [error, setError] = useState<string | null>(null);
	const channels = Array.from({ length: channelCount }, (_, index) => index);

	const update = (id: string, patch: Partial<MeasurementPair>) => {
		const next = routing.pairs.map((pair) => (pair.id === id ? { ...pair, ...patch } : pair));
		const changed = next.find((pair) => pair.id === id);
		const problem = changed ? measurementProblem(changed, channelCount) : null;
		setError(problem);
		if (problem) return;
		onChange({ pairs: next });
	};

	const add = () => {
		setError(null);
		onChange({ pairs: [...routing.pairs, newMeasurement(routing.pairs, channelCount)] });
	};

	const remove = (id: string) => {
		setError(null);
		if (viewedPairId === id) onView(null);
		onChange({ pairs: routing.pairs.filter((pair) => pair.id !== id) });
	};

	const header = (
		<div className="flex flex-shrink-0 items-start justify-between gap-4">
			<div className="flex flex-col gap-1">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Measurements
				</h2>
				<p className="max-w-prose text-xs text-muted-foreground">
					Each measurement compares one microphone input with a reference. Add one per mic position
					to measure several at once, then pick which one the Transfer Function view shows.
				</p>
			</div>
			<button
				type="button"
				data-testid="add-measurement"
				onClick={add}
				disabled={routing.pairs.length >= MAX_PAIRS}
				className="flex flex-shrink-0 items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-semibold text-primary-foreground hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
			>
				<svg
					aria-hidden="true"
					width="14"
					height="14"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					strokeWidth="2.5"
					strokeLinecap="round"
				>
					<path d="M12 5v14M5 12h14" />
				</svg>
				Add measurement
			</button>
		</div>
	);

	if (isMultiWindow) {
		return (
			<section data-testid="measurement-list" className="flex h-full min-h-0 flex-col gap-3">
				{header}
				<p className="text-sm text-muted-foreground">
					Paused while multi-window resolution is on: it shares one processor across every
					measurement. Switch back to single-FFT mode in Settings to measure each one separately.
				</p>
			</section>
		);
	}

	return (
		<section data-testid="measurement-list" className="flex h-full min-h-0 flex-col gap-3">
			{header}

			{channelCount < 2 && (
				<p className="flex-shrink-0 rounded-md border border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
					The stream opens {channelCount} input channel. To measure several microphones, or to use
					an input as the reference, raise <span className="text-foreground">Input channels</span>{" "}
					in the sidebar before starting the stream.
				</p>
			)}

			{error && (
				<p role="alert" className="flex-shrink-0 text-xs text-red-500">
					{error}
				</p>
			)}

			{routing.pairs.length === 0 ? (
				<div className="flex flex-1 flex-col items-center justify-center gap-1 rounded-md border border-dashed border-border px-6 py-8 text-center">
					<p className="text-sm text-foreground">No measurements yet</p>
					<p className="max-w-sm text-xs text-muted-foreground">
						The Transfer Function view measures the sidebar's input against the generator. Add a
						measurement here to compare more inputs, or to use another input as the reference.
					</p>
				</div>
			) : (
				<div className="min-h-0 flex-1 overflow-auto rounded-md border border-border">
					<table className="w-full border-collapse text-left text-xs">
						<thead className="sticky top-0 z-10 bg-card text-muted-foreground">
							<tr>
								<th scope="col" className="px-3 py-2 font-medium">
									On
								</th>
								<th scope="col" className="px-2 py-2 font-medium">
									Name
								</th>
								<th scope="col" className="px-2 py-2 font-medium">
									Microphone input
								</th>
								<th scope="col" className="px-2 py-2 font-medium">
									Reference
								</th>
								<th scope="col" className="px-2 py-2 font-medium">
									Delay (samples)
								</th>
								<th scope="col" className="px-2 py-2 font-medium">
									Show
								</th>
								<th scope="col" className="px-2 py-2">
									<span className="sr-only">Remove</span>
								</th>
							</tr>
						</thead>
						<tbody>
							{routing.pairs.map((pair) => {
								const problem = measurementProblem(pair, channelCount);
								return (
									<tr
										key={pair.id}
										className={`border-t border-border/60 ${pair.enabled ? "" : "text-muted-foreground"}`}
									>
										<td className="px-3 py-1.5">
											<input
												type="checkbox"
												checked={pair.enabled}
												onChange={(e) => update(pair.id, { enabled: e.target.checked })}
												aria-label={`Measure ${pair.name}`}
												className="h-4 w-4 accent-primary"
											/>
										</td>
										<td className="px-2 py-1.5">
											<input
												type="text"
												value={pair.name}
												onChange={(e) => update(pair.id, { name: e.target.value })}
												aria-label="Measurement name"
												className={`${fieldClass} w-32`}
											/>
										</td>
										<td className="px-2 py-1.5">
											<select
												value={pair.measurement_channel}
												onChange={(e) =>
													update(pair.id, { measurement_channel: Number(e.target.value) })
												}
												aria-label={`Microphone input for ${pair.name}`}
												className={fieldClass}
											>
												{channels.map((channel) => (
													<option key={channel} value={channel}>
														Input {channel + 1}
													</option>
												))}
											</select>
										</td>
										<td className="px-2 py-1.5">
											<select
												value={referenceValue(pair.reference)}
												onChange={(e) =>
													update(pair.id, { reference: parseReference(e.target.value) })
												}
												aria-label={`Reference for ${pair.name}`}
												className={fieldClass}
											>
												<option value="generator">Generator</option>
												{channels.map((channel) => (
													<option key={channel} value={`input:${channel}`}>
														Input {channel + 1}
													</option>
												))}
											</select>
										</td>
										<td className="px-2 py-1.5">
											<input
												type="number"
												min={0}
												step={1}
												value={pair.delay_samples}
												onChange={(e) =>
													update(pair.id, {
														delay_samples: Math.max(0, Math.round(Number(e.target.value) || 0)),
													})
												}
												aria-label={`Delay for ${pair.name} in samples`}
												className={`${fieldClass} w-20 font-mono tabular-nums`}
											/>
										</td>
										<td className="px-2 py-1.5">
											<input
												type="radio"
												name="viewed-measurement"
												checked={viewedPairId === pair.id}
												disabled={!pair.enabled || problem !== null}
												onChange={() => onView(pair.id)}
												aria-label={`Show ${pair.name} in the Transfer Function view`}
												className="h-4 w-4 accent-primary"
											/>
										</td>
										<td className="px-2 py-1.5 text-right">
											<button
												type="button"
												onClick={() => remove(pair.id)}
												aria-label={`Remove ${pair.name}`}
												title="Remove"
												className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
											>
												<svg
													aria-hidden="true"
													width="14"
													height="14"
													viewBox="0 0 24 24"
													fill="none"
													stroke="currentColor"
													strokeWidth="2"
													strokeLinecap="round"
													strokeLinejoin="round"
												>
													<path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6" />
												</svg>
											</button>
										</td>
									</tr>
								);
							})}
						</tbody>
					</table>
				</div>
			)}
		</section>
	);
};
