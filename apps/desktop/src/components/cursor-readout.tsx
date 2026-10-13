import { type CursorSample, formatDelta } from "../lib/plot-scales";

interface CursorReadoutProps {
	cursorA: CursorSample | null;
	cursorB: CursorSample | null;
}

const formatFrequency = (hz: number): string => {
	const abs = Math.abs(hz);
	return abs >= 1000 ? `${(hz / 1000).toFixed(2)} kHz` : `${hz.toFixed(1)} Hz`;
};

const formatSigned = (value: number, digits: number, unit: string): string =>
	`${value >= 0 ? "+" : ""}${value.toFixed(digits)} ${unit}`;

interface CursorValueProps {
	label: string;
	sample: CursorSample | null;
	swatchClassName: string;
}

const CursorValue = ({ label, sample, swatchClassName }: CursorValueProps) => (
	<span className="flex items-center gap-1.5">
		<span className={`w-2 h-2 rounded-full ${swatchClassName}`} />
		<span className="text-muted-foreground">{label}</span>
		{sample ? (
			<span className="font-mono">
				{formatFrequency(sample.frequencyHz)} · {sample.magnitudeDb.toFixed(1)} dB ·{" "}
				{sample.groupDelayMs.toFixed(2)} ms
			</span>
		) : (
			<span className="text-muted-foreground/60">not placed</span>
		)}
	</span>
);

/** Dual-cursor readout for a measurement plot: cursor A, cursor B, and the delta between them. */
export const CursorReadout = ({ cursorA, cursorB }: CursorReadoutProps) => {
	const delta = cursorA && cursorB ? formatDelta(cursorA, cursorB) : null;

	return (
		<div data-testid="cursor-readout" className="flex flex-wrap items-center gap-4 text-xs">
			<CursorValue label="Cursor A (click)" sample={cursorA} swatchClassName="bg-amber-500" />
			<CursorValue
				label="Cursor B (shift+click)"
				sample={cursorB}
				swatchClassName="bg-fuchsia-500"
			/>
			{delta && (
				<span className="flex items-center gap-1.5 text-muted-foreground">
					<span className="font-semibold">Δ</span>
					<span className="font-mono">
						{formatSigned(delta.deltaFrequencyHz, 1, "Hz")} · {formatSigned(delta.deltaDb, 1, "dB")}{" "}
						· {formatSigned(delta.deltaMs, 2, "ms")}
					</span>
				</span>
			)}
		</div>
	);
};
