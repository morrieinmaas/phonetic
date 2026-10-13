import type { RcQuality, RcResult, RoomCriteriaFamily, RoomCriteriaResult } from "../types/audio";

const OCTAVE_BAND_HZ = [63, 125, 250, 500, 1000, 2000, 4000, 8000] as const;

/** RC Mark II's own nine octave bands — wider and lower than the NC/NR set above. */
const RC_OCTAVE_BAND_HZ = [16, 31.5, 63, 125, 250, 500, 1000, 2000, 4000] as const;

const FAMILY_OPTIONS: { value: RoomCriteriaFamily; label: string }[] = [
	{ value: "nc", label: "NC — Noise Criteria" },
	{ value: "nr", label: "NR — Noise Rating" },
	{ value: "rc", label: "RC — Room Criterion" },
];

/** What each RC Mark II quality letter means, spelled out — "LF" alone means nothing to most operators. */
const RC_QUALITY_INFO: Record<RcQuality, { code: string; meaning: string }> = {
	neutral: { code: "N", meaning: "neutral" },
	low_frequency: { code: "LF", meaning: "low-frequency rumble" },
	mid_frequency: { code: "MF", meaning: "mid-frequency roar" },
	high_frequency: { code: "HF", meaning: "high-frequency hiss" },
};

interface RoomCriteriaPanelProps {
	/** Which family is selected. Falls back to `result`'s family when omitted, so callers that
	 * only ever show NC/NR (and existing tests) don't need to pass it. */
	family?: RoomCriteriaFamily;
	result: RoomCriteriaResult | null;
	rcResult?: RcResult | null;
	onFamilyChange: (family: RoomCriteriaFamily) => void;
}

const formatDb = (value: number | undefined): string =>
	value === undefined ? "—" : value.toFixed(1);

/** Rating readout: the family's letter code plus the tangency number, e.g. "NC-30". */
const RatingHeadline = ({ result }: { result: RoomCriteriaResult }) => (
	<div className="flex items-baseline gap-3">
		<span className="font-mono text-3xl font-bold text-foreground tabular-nums">
			{`${result.family.toUpperCase()}-${result.rating}`}
		</span>
		<span className="text-xs text-muted-foreground">
			Exceeded by the {result.exceeded_band_hz} Hz band
		</span>
	</div>
);

/**
 * RC readout: the level and the quality letter together, e.g. "RC-35 (LF)", plus the
 * letter's meaning spelled out in words.
 */
const RcHeadline = ({ result }: { result: RcResult }) => {
	const info = RC_QUALITY_INFO[result.quality];
	return (
		<div className="flex items-baseline gap-3">
			<span className="font-mono text-3xl font-bold text-foreground tabular-nums">
				{`RC-${Math.round(result.level)} (${info.code})`}
			</span>
			<span className="text-xs text-muted-foreground">{info.meaning}</span>
		</div>
	);
};

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
			<span
				className={`font-mono text-sm tabular-nums ${exceedsCurve ? "text-red-400" : "text-foreground"}`}
			>
				{formatDb(levelDb)}
			</span>
			<span className="font-mono text-[10px] tabular-nums text-muted-foreground/70">
				/{formatDb(curveDb)}
			</span>
		</div>
	);
};

interface RcBandCellProps {
	hz: number;
	levelDb: number | undefined;
	referenceDb: number | undefined;
	bins: number | undefined;
	/** Comes from the backend with the rating, so the panel marks the same
	 *  bands thin that `under_resolved` was computed from. */
	minBins: number;
}

/** One RC band: measured level over the reference line, plus its FFT bin count — the number
 * that tells the operator whether this band's reading means anything. */
const RcBandCell = ({ hz, levelDb, referenceDb, bins, minBins }: RcBandCellProps) => {
	const thin = bins !== undefined && bins < minBins;

	return (
		<div
			className={`flex flex-col items-center gap-0.5 rounded border px-1 py-1.5 ${
				thin ? "border-amber-500/60 bg-amber-500/10" : "border-border/50"
			}`}
		>
			<span className="font-mono text-[10px] text-muted-foreground">{hz}</span>
			<span className="font-mono text-sm tabular-nums text-foreground">{formatDb(levelDb)}</span>
			<span className="font-mono text-[10px] tabular-nums text-muted-foreground/70">
				/{formatDb(referenceDb)}
			</span>
			<span
				className={`font-mono text-[9px] tabular-nums ${thin ? "text-amber-400" : "text-muted-foreground/60"}`}
			>
				{bins ?? "—"} bin{bins === 1 ? "" : "s"}
			</span>
		</div>
	);
};

/**
 * Prominent, plain-language guidance for when RC's low bands don't have enough FFT bins to
 * back a trustworthy reading. This is guidance, not an error — the mid/high-frequency reading
 * is still usable — so it uses the same amber warning treatment as `DiagnosticsBanner`, not red.
 */
const UnderResolvedWarning = () => (
	<div
		data-testid="rc-under-resolved-warning"
		className="rounded border border-amber-500/60 bg-card px-3 py-2 text-[11px] leading-snug text-amber-400"
	>
		<p className="font-semibold">Low-frequency bands are under-resolved</p>
		<p>
			At the current FFT size, one or more bands below 63 Hz don't contain enough FFT bins for a
			reliable reading (per-band bin counts are shown below) — the low-frequency classification
			above may not be trustworthy. Raise the FFT size in Settings to resolve them. The mid- and
			high-frequency reading is still usable as-is.
		</p>
	</div>
);

/**
 * RC Mark II content: the level+quality headline, the under-resolved warning when it applies,
 * and the nine RC bands against the reference line.
 */
const RcContent = ({ result }: { result: RcResult }) => (
	<>
		<RcHeadline result={result} />

		{result.under_resolved && <UnderResolvedWarning />}

		<div className="grid grid-cols-9 gap-1">
			{RC_OCTAVE_BAND_HZ.map((hz, i) => (
				<RcBandCell
					key={hz}
					hz={hz}
					levelDb={result.band_levels_db[i]}
					referenceDb={result.reference_db[i]}
					bins={result.bins_per_band[i]}
					minBins={result.min_bins_per_band}
				/>
			))}
		</div>

		<p className="text-[11px] leading-snug text-muted-foreground">
			This is a measured value, not a certified compliance rating — treat it as a field estimate
			against the RC Mark II reference line, not a substitute for calibrated acoustical testing.
		</p>
	</>
);

/**
 * Shows a room criteria rating: the tangency rating, the octave band that determined it,
 * every measured octave band against the selected curve, and the family selector.
 *
 * RC Mark II is not a tangency rating (see `RcResult`), so selecting it swaps in `RcContent`
 * instead: a level+quality headline, an under-resolved warning when the low bands lack FFT
 * bins, and RC's own nine-band grid rather than the NC/NR eight.
 *
 * Layout: this panel declares its own minimum height rather than a `min-h-0` that relies on
 * an ancestor. There is no ResizeObserver here — the content is a fixed grid, not a chart, so
 * it never needs one.
 */
export const RoomCriteriaPanel = ({
	family,
	result,
	rcResult,
	onFamilyChange,
}: RoomCriteriaPanelProps) => {
	const selectedFamily: RoomCriteriaFamily = family ?? result?.family ?? "nc";
	const isRc = selectedFamily === "rc";

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
					onChange={(e) => onFamilyChange(e.target.value as RoomCriteriaFamily)}
					className="rounded border border-border bg-card px-2 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
				>
					{FAMILY_OPTIONS.map((option) => (
						<option key={option.value} value={option.value}>
							{option.label}
						</option>
					))}
				</select>
			</div>

			{isRc ? (
				rcResult ? (
					<RcContent result={rcResult} />
				) : (
					<p className="text-xs text-muted-foreground">No measurement yet.</p>
				)
			) : result ? (
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
