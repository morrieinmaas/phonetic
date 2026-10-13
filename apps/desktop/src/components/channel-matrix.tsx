import { useState } from "react";
import type { MeasurementPair, ReferenceSource, Routing } from "../types/audio";

interface ChannelMatrixProps {
	routing: Routing;
	/** How many input channels the current device/stream exposes; the grid has one row per channel. */
	channelCount: number;
	/** Called with the whole updated routing once a reference change passes validation. */
	onChange: (routing: Routing) => void;
	/**
	 * Multi-window mode shares a single processor across every pair (Controller ruling B8), so
	 * per-pair data would just repeat the same trace under every pair's id. Rather than let the
	 * matrix look live while it silently isn't, it shows a paused message instead of the grid.
	 */
	isMultiWindow?: boolean;
}

const parseReferenceValue = (value: string): ReferenceSource =>
	value === "generator"
		? { source: "generator" }
		: { source: "input", channel: Number(value.slice("input:".length)) };

const referenceOptionValue = (reference: ReferenceSource): string =>
	reference.source === "generator" ? "generator" : `input:${reference.channel}`;

/** Full-word summary of a pair's current reference, shown once per pair. */
const referenceSummary = (reference: ReferenceSource): string =>
	reference.source === "generator" ? "Generator" : `Input ${reference.channel + 1}`;

/**
 * Channel-by-pair routing matrix. Rows are input channels rather than pairs: the operator's
 * question is "what is channel 12 doing?", and with rows-as-channels that is answered by one
 * row, without scanning across up to 32 channel columns to find the answer.
 *
 * Layout: this panel declares its own minimum height and flex-column layout; the grid scrolls
 * *inside* it via the `data-matrix-scroll` container below, which carries its own `min-h-0`
 * rather than a pixel minimum — never both on the panel and the scroll container, or their
 * minimums fight and the page grows instead of the grid scrolling.
 */
export const ChannelMatrix = ({
	routing,
	channelCount,
	onChange,
	isMultiWindow = false,
}: ChannelMatrixProps) => {
	const [error, setError] = useState<string | null>(null);

	const handleReferenceChange = (pair: MeasurementPair, value: string) => {
		const reference = parseReferenceValue(value);
		if (reference.source === "input" && reference.channel === pair.measurement_channel) {
			setError(`"${pair.name}" cannot reference itself — pick a different input channel.`);
			return;
		}
		setError(null);
		onChange({
			pairs: routing.pairs.map((candidate) =>
				candidate.id === pair.id ? { ...candidate, reference } : candidate,
			),
		});
	};

	if (isMultiWindow) {
		return (
			<section
				data-testid="channel-matrix"
				className="glass flex min-h-[320px] flex-col items-center justify-center gap-1 rounded-lg p-4 text-center"
			>
				<p className="text-sm font-medium text-foreground">Paused — multi-window active</p>
				<p className="max-w-sm text-xs text-muted-foreground">
					Multi-window resolution mode shares one processor across every pair. Switch back to
					single-FFT mode to resume per-pair measurement.
				</p>
			</section>
		);
	}

	const channels = Array.from({ length: channelCount }, (_, index) => index);

	return (
		<section
			data-testid="channel-matrix"
			className="glass flex min-h-[320px] flex-1 flex-col gap-2 rounded-lg p-4"
		>
			<h2 className="flex-shrink-0 text-sm font-semibold uppercase tracking-wider text-muted-foreground">
				Channel Matrix
			</h2>

			{error && (
				<p role="alert" className="flex-shrink-0 text-xs text-red-400">
					{error}
				</p>
			)}

			<div
				data-matrix-scroll
				className="min-h-0 flex-1 overflow-y-auto overflow-x-auto rounded-md border border-border"
			>
				<table className="w-full border-collapse text-left text-sm">
					<thead>
						<tr>
							<th
								scope="col"
								className="sticky top-0 z-10 bg-card px-2 py-1.5 text-xs font-medium uppercase tracking-wider text-muted-foreground"
							>
								Channel
							</th>
							{routing.pairs.map((pair) => (
								<th
									key={pair.id}
									scope="col"
									className={`sticky top-0 z-10 bg-card px-2 py-1.5 align-top ${
										pair.enabled ? "" : "opacity-50"
									}`}
								>
									<div className="flex flex-col gap-1">
										<span className="text-xs font-semibold text-foreground">{pair.name}</span>
										<select
											aria-label={`Reference for ${pair.name}`}
											value={referenceOptionValue(pair.reference)}
											onChange={(event) => handleReferenceChange(pair, event.target.value)}
											className="rounded border border-border bg-card px-1 py-0.5 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
										>
											<option value="generator">Gen</option>
											{channels.map((channel) => (
												<option key={channel} value={`input:${channel}`}>
													{`Ch ${channel + 1}`}
												</option>
											))}
										</select>
										<span className="text-[11px] font-normal normal-case text-muted-foreground">
											{referenceSummary(pair.reference)}
										</span>
									</div>
								</th>
							))}
						</tr>
					</thead>
					<tbody>
						{channels.map((channel) => (
							<tr key={channel} className="border-t border-border/50 hover:bg-muted/40">
								<td className="px-2 py-1 font-mono text-foreground">{channel + 1}</td>
								{routing.pairs.map((pair) => {
									const isMeasurement = pair.measurement_channel === channel;
									const isReference =
										pair.reference.source === "input" && pair.reference.channel === channel;
									return (
										<td key={pair.id} className="px-2 py-1 text-center">
											{isMeasurement && (
												<span
													className="font-mono text-xs font-semibold text-primary"
													aria-label={`Measurement channel for ${pair.name}`}
												>
													M
												</span>
											)}
											{isReference && (
												<span
													className="font-mono text-xs font-semibold text-accent"
													aria-label={`Reference channel for ${pair.name}`}
												>
													R
												</span>
											)}
											{!isMeasurement && !isReference && (
												<span className="text-muted-foreground/40">—</span>
											)}
										</td>
									);
								})}
							</tr>
						))}
					</tbody>
				</table>
			</div>
		</section>
	);
};
