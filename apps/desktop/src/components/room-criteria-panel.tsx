import type { RcQuality, RcResult, RoomCriteriaFamily, RoomCriteriaResult } from "../types/audio";

const OCTAVE_BAND_HZ = [63, 125, 250, 500, 1000, 2000, 4000, 8000] as const;

/** RC Mark II's own nine octave bands, wider and lower than the NC/NR set above. */
const RC_OCTAVE_BAND_HZ = [16, 31.5, 63, 125, 250, 500, 1000, 2000, 4000] as const;

const FAMILY_OPTIONS: { value: RoomCriteriaFamily; label: string }[] = [
	{ value: "nc", label: "NC (Noise Criteria, US)" },
	{ value: "nr", label: "NR (Noise Rating, EU/UK)" },
	{ value: "rc", label: "RC Mark II (HVAC sound quality)" },
];

/** What each RC Mark II quality letter means, spelled out; "LF" alone means nothing to most operators. */
const RC_QUALITY_INFO: Record<RcQuality, { code: string; meaning: string }> = {
	neutral: { code: "N", meaning: "neutral, well balanced" },
	low_frequency: { code: "LF", meaning: "low-frequency rumble" },
	mid_frequency: { code: "MF", meaning: "mid-frequency roar" },
	high_frequency: { code: "HF", meaning: "high-frequency hiss" },
};

/**
 * Typical design targets for background noise, as commonly published (for example in the
 * ASHRAE Handbook's guidance tables). Approximate, and the same order of magnitude for NC, NR
 * and RC; they give a rating meaning without claiming a requirement.
 */
export const TYPICAL_TARGETS: { upTo: number; spaces: string }[] = [
	{ upTo: 20, spaces: "concert halls, recording studios" },
	{ upTo: 25, spaces: "theatres, broadcast studios" },
	{ upTo: 30, spaces: "bedrooms, classrooms, meeting rooms" },
	{ upTo: 35, spaces: "private offices, libraries" },
	{ upTo: 40, spaces: "open-plan offices, restaurants" },
	{ upTo: 50, spaces: "lobbies, shops, kitchens" },
];

/** The kind of room a rating is typical for, or null when it is louder than any of them. */
export const typicalFor = (rating: number): string | null =>
	TYPICAL_TARGETS.find((target) => rating <= target.upTo)?.spaces ?? null;

interface RoomCriteriaPanelProps {
	/** Which family is selected. Falls back to `result`'s family when omitted. */
	family?: RoomCriteriaFamily;
	result: RoomCriteriaResult | null;
	rcResult?: RcResult | null;
	onFamilyChange: (family: RoomCriteriaFamily) => void;
}

interface BandChartProps {
	bands: readonly number[];
	levels: number[];
	curve: number[];
	curveLabel: string;
	/** Bands to draw in the warning colour, e.g. RC bands with too few FFT bins. */
	thinBands?: boolean[];
	highlightHz?: number;
	/**
	 * Colour bands above the curve red. Right for NC/NR, where that means the room is louder
	 * than the rating; wrong for RC, where deviation from the line describes sound quality.
	 */
	markOver?: boolean;
}

/**
 * Measured octave-band levels as columns against the rating curve as a line. Seeing which
 * columns reach the line is the whole idea of a tangency rating; a grid of numbers hid it.
 */
const BandChart = ({
	bands,
	levels,
	curve,
	curveLabel,
	thinBands,
	highlightHz,
	markOver = true,
}: BandChartProps) => {
	const values = [...levels, ...curve].filter(Number.isFinite);
	const lo = Math.floor((Math.min(...values) - 5) / 10) * 10;
	const hi = Math.ceil((Math.max(...values) + 5) / 10) * 10;
	const width = 100 / bands.length;
	const y = (db: number) => 100 - ((db - lo) / (hi - lo || 1)) * 100;
	const curvePoints = curve.map((db, i) => `${(i + 0.5) * width},${y(db)}`).join(" ");

	return (
		<figure className="flex min-h-[180px] flex-1 flex-col gap-1">
			<div className="relative min-h-0 flex-1">
				<svg
					viewBox="0 0 100 100"
					preserveAspectRatio="none"
					className="absolute inset-0 h-full w-full overflow-visible"
					role="img"
					aria-label={`Octave band levels against the ${curveLabel} curve`}
				>
					{bands.map((hz, i) => {
						const level = levels[i] ?? lo;
						const limit = curve[i] ?? hi;
						const over = markOver && level > limit;
						const fill = over
							? "fill-red-500/70"
							: thinBands?.[i]
								? "fill-amber-500/50"
								: hz === highlightHz
									? "fill-primary"
									: "fill-primary/45";
						return (
							<rect
								key={hz}
								x={i * width + width * 0.18}
								width={width * 0.64}
								y={y(level)}
								height={Math.max(0, 100 - y(level))}
								className={fill}
							/>
						);
					})}
					<polyline
						points={curvePoints}
						fill="none"
						className="stroke-foreground"
						strokeWidth="2"
						vectorEffect="non-scaling-stroke"
						strokeLinejoin="round"
					/>
				</svg>
				{/* Values as HTML so the stretched SVG cannot distort the digits. */}
				{bands.map((hz, i) => (
					<span
						key={hz}
						className="absolute -translate-x-1/2 pt-0.5 font-mono text-xs tabular-nums text-foreground"
						style={{ left: `${(i + 0.5) * width}%`, top: `${y(levels[i] ?? lo)}%` }}
					>
						{(levels[i] ?? 0).toFixed(0)}
					</span>
				))}
			</div>
			<div className="grid" style={{ gridTemplateColumns: `repeat(${bands.length}, 1fr)` }}>
				{bands.map((hz) => (
					<span
						key={hz}
						className={`text-center font-mono text-xs ${hz === highlightHz ? "font-semibold text-foreground" : "text-muted-foreground"}`}
					>
						{hz >= 1000 ? `${hz / 1000}k` : hz}
					</span>
				))}
			</div>
			<figcaption className="flex items-center gap-4 text-xs text-muted-foreground">
				<span className="flex items-center gap-1.5">
					<span className="h-2.5 w-2.5 rounded-sm bg-primary/45" />
					Measured, dB SPL per octave band (Hz)
				</span>
				<span className="flex items-center gap-1.5">
					<span className="h-0.5 w-4 bg-foreground" />
					{curveLabel}
				</span>
			</figcaption>
		</figure>
	);
};

const Headline = ({ title, detail }: { title: string; detail: string }) => (
	<div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
		<span className="font-mono text-3xl font-bold tabular-nums text-foreground">{title}</span>
		<span className="text-sm text-muted-foreground">{detail}</span>
	</div>
);

const TangencyContent = ({ result }: { result: RoomCriteriaResult }) => {
	const name = result.family.toUpperCase();
	const offScale = result.band_levels_db.some(
		(level, i) => level > (result.curve_db[i] ?? Number.POSITIVE_INFINITY),
	);
	const typical = typicalFor(result.rating);

	return (
		<>
			<Headline
				title={offScale ? `Above ${name}-${result.rating}` : `${name}-${result.rating}`}
				detail={
					offScale
						? `Louder than the highest ${name} curve; the ${result.exceeded_band_hz} Hz band is furthest over.`
						: `Set by the ${result.exceeded_band_hz} Hz band, the one closest to its limit.${typical ? ` Typical for ${typical}.` : ""}`
				}
			/>
			<BandChart
				bands={OCTAVE_BAND_HZ}
				levels={result.band_levels_db}
				curve={result.curve_db}
				curveLabel={`${name} rating curve`}
				highlightHz={result.exceeded_band_hz}
			/>
		</>
	);
};

/**
 * Plain-language guidance for when RC's low bands don't have enough FFT bins to back a
 * trustworthy reading. Guidance, not an error: the mid and high reading still stands.
 */
const UnderResolvedWarning = () => (
	<div
		data-testid="rc-under-resolved-warning"
		className="rounded-md border border-amber-500/60 px-3 py-2 text-xs leading-snug text-foreground"
	>
		<span className="font-semibold text-amber-500">Low-frequency bands are under-resolved. </span>
		The amber bands have too few FFT bins for a reliable reading, so the LF part of the rating may
		be off. Raise the FFT size in Settings; the mid and high reading is fine as is.
	</div>
);

const RcContent = ({ result }: { result: RcResult }) => {
	const info = RC_QUALITY_INFO[result.quality];
	const typical = typicalFor(result.level);
	return (
		<>
			<Headline
				title={`RC-${Math.round(result.level)} (${info.code})`}
				detail={`${info.meaning[0]?.toUpperCase()}${info.meaning.slice(1)}.${typical ? ` Level typical for ${typical}.` : ""}`}
			/>
			{result.under_resolved && <UnderResolvedWarning />}
			<BandChart
				bands={RC_OCTAVE_BAND_HZ}
				levels={result.band_levels_db}
				curve={result.reference_db}
				curveLabel="RC reference line"
				markOver={false}
				thinBands={result.bins_per_band.map((bins) => bins < result.min_bins_per_band)}
			/>
		</>
	);
};

/**
 * Room criteria: how loud a room's steady background noise is (air conditioning, traffic,
 * equipment), as a single number. The rating is the lowest standard curve that every octave
 * band stays under, so lower is quieter, and the chart shows which band sets it.
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
		<section data-testid="room-criteria-panel" className="flex h-full min-h-0 flex-col gap-3">
			<div className="flex flex-shrink-0 items-start justify-between gap-4">
				<div className="flex flex-col gap-1">
					<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
						Room Criteria
					</h2>
					<p className="max-w-prose text-xs text-muted-foreground">
						Rates a room's steady background noise, such as air conditioning or traffic, with the
						generator off. Lower is quieter. Needs a calibrated microphone.
					</p>
				</div>
				<select
					aria-label="Curve family"
					value={selectedFamily}
					onChange={(e) => onFamilyChange(e.target.value as RoomCriteriaFamily)}
					className="flex-shrink-0 rounded border border-border bg-card px-2 py-1 text-xs text-foreground"
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
					<p className="text-sm text-muted-foreground">Start the stream to rate the room.</p>
				)
			) : result ? (
				<TangencyContent result={result} />
			) : (
				<p className="text-sm text-muted-foreground">Start the stream to rate the room.</p>
			)}

			<p className="flex-shrink-0 text-xs text-muted-foreground">
				A measured value for field use, not a certified acoustic test.
			</p>
		</section>
	);
};
