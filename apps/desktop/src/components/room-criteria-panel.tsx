import type { CurveFamily, RoomCriteriaResult } from "../types/audio";

const OCTAVE_BAND_HZ = [63, 125, 250, 500, 1000, 2000, 4000, 8000] as const;

const FAMILY_OPTIONS: { value: CurveFamily; label: string }[] = [
	{ value: "nc", label: "NC — Noise Criteria" },
	{ value: "nr", label: "NR — Noise Rating" },
];

interface RoomCriteriaPanelProps {
	result: RoomCriteriaResult | null;
	onFamilyChange: (family: CurveFamily) => void;
}

const formatDb = (value: number | undefined): string =>
	value === undefined ? "—" : value.toFixed(1);

/** Rating readout: the family's letter code plus the tangency number, e.g. "NC-30". */
const RatingHeadline = ({ result }: { result: RoomCriteriaResult }) => (
	<div className="flex items-baseline gap-3">
		<span className="font-mono text-3xl font-bold text-foreground">
			{`${result.family.toUpperCase()}-${result.rating}`}
		</span>
		<span className="text-xs text-muted-foreground">
			Exceeded by the {result.exceeded_band_hz} Hz band
		</span>
	</div>
);

interface BandCellProps {
	hz: number;
	levelDb: number | undefined;
	curveDb: number | undefined;
	isDeterminingBand: boolean;
}

const BandCell = ({ hz, levelDb, curveDb, isDeterminingBand }: BandCellProps) => {
	const exceedsCurve = levelDb !== undefined && curveDb !== undefined && levelDb > curveDb;

	return (
		<div
			className={`flex flex-col items-center gap-0.5 rounded border px-1 py-1.5 ${
				isDeterminingBand ? "border-primary bg-primary/10" : "border-border/50"
			}`}
		>
			<span className="font-mono text-[10px] text-muted-foreground">{hz}</span>
			<span className={`font-mono text-sm ${exceedsCurve ? "text-red-400" : "text-foreground"}`}>
				{formatDb(levelDb)}
			</span>
			<span className="font-mono text-[10px] text-muted-foreground/70">/{formatDb(curveDb)}</span>
		</div>
	);
};

/**
 * Shows a room criteria rating: the tangency rating, the octave band that determined it,
 * every measured octave band against the selected curve, and the family selector.
 *
 * Layout: this panel declares its own minimum height rather than a `min-h-0` that relies on
 * an ancestor. There is no ResizeObserver here — the content is a fixed eight-cell grid, not a
 * chart, so it never needs one.
 */
export const RoomCriteriaPanel = ({ result, onFamilyChange }: RoomCriteriaPanelProps) => {
	const selectedFamily = result?.family ?? "nc";

	return (
		<section
			data-testid="room-criteria-panel"
			className="glass flex min-h-[260px] flex-col gap-3 rounded-lg p-4"
		>
			<div className="flex items-center justify-between gap-3">
				<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
					Room Criteria
				</h2>
				<select
					aria-label="Curve family"
					value={selectedFamily}
					onChange={(e) => onFamilyChange(e.target.value as CurveFamily)}
					className="rounded border border-border bg-card px-2 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
				>
					{FAMILY_OPTIONS.map((option) => (
						<option key={option.value} value={option.value}>
							{option.label}
						</option>
					))}
				</select>
			</div>

			{result ? (
				<>
					<RatingHeadline result={result} />

					<div className="grid grid-cols-8 gap-1">
						{OCTAVE_BAND_HZ.map((hz, i) => (
							<BandCell
								key={hz}
								hz={hz}
								levelDb={result.band_levels_db[i]}
								curveDb={result.curve_db[i]}
								isDeterminingBand={hz === result.exceeded_band_hz}
							/>
						))}
					</div>

					<p className="text-[11px] leading-snug text-muted-foreground">
						This is a measured value, not a certified compliance rating — treat it as a field
						estimate against the {result.family.toUpperCase()} curve, not a substitute for
						calibrated acoustical testing.
					</p>
				</>
			) : (
				<p className="text-xs text-muted-foreground">No measurement yet.</p>
			)}
		</section>
	);
};
