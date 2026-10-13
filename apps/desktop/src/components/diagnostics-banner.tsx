import { useEffect, useState } from "react";
import { audioApi } from "../services/audio-api";
import { applyMeasuredDelay } from "../services/delay-apply";
import { useAudioStore } from "../stores/audio-store";
import type { Diagnosis } from "../types/audio";

const SEVERITY_RANK: Record<Diagnosis["severity"], number> = {
	error: 3,
	warning: 2,
	info: 1,
};

/** Highest-severity diagnosis the operator has not dismissed. */
export const selectTopDiagnosis = (
	diagnoses: Diagnosis[],
	dismissed: Set<string>,
): Diagnosis | null => {
	const live = diagnoses.filter((d) => !dismissed.has(d.code));
	if (live.length === 0) {
		return null;
	}
	return live.reduce((best, d) =>
		SEVERITY_RANK[d.severity] > SEVERITY_RANK[best.severity] ? d : best,
	);
};

/** A diagnosis on screen, with how many polls in a row it has been absent. */
export interface ShownDiagnosis {
	diagnosis: Diagnosis;
	misses: number;
}

/**
 * How many consecutive clean polls hide a diagnosis. One clean poll is not
 * enough: coherence hovering at the threshold would make the toast blink.
 */
export const CLEAR_AFTER_POLLS = 2;

/**
 * Merge the latest poll into what is on screen. A diagnosis the backend still
 * reports is refreshed; one it stopped reporting stays until it has been
 * absent for CLEAR_AFTER_POLLS polls, then disappears on its own.
 */
export const reconcileDiagnoses = (
	shown: ShownDiagnosis[],
	latest: Diagnosis[],
): ShownDiagnosis[] => {
	const latestCodes = new Set(latest.map((d) => d.code));
	const fading = shown
		.filter((s) => !latestCodes.has(s.diagnosis.code))
		.map((s) => ({ ...s, misses: s.misses + 1 }))
		.filter((s) => s.misses < CLEAR_AFTER_POLLS);
	return [...latest.map((diagnosis) => ({ diagnosis, misses: 0 })), ...fading];
};

/**
 * Forget a dismissal once its problem has gone away, so the same fault
 * coming back later is reported again instead of being silenced forever.
 */
export const rearmDismissed = (dismissed: Set<string>, shown: ShownDiagnosis[]): Set<string> => {
	const stillShown = new Set(shown.map((s) => s.diagnosis.code));
	const next = new Set([...dismissed].filter((code) => stillShown.has(code)));
	return next.size === dismissed.size ? dismissed : next;
};

const POLL_MS = 1000;

const SeverityIcon = ({ severity }: { severity: Diagnosis["severity"] }) => (
	<svg
		aria-hidden="true"
		width="18"
		height="18"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		strokeWidth="2"
		strokeLinecap="round"
		strokeLinejoin="round"
		className="mt-0.5 flex-shrink-0"
	>
		{severity === "error" ? (
			<>
				<circle cx="12" cy="12" r="10" />
				<path d="M12 8v4M12 16h.01" />
			</>
		) : (
			<>
				<path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" />
				<path d="M12 9v4M12 17h.01" />
			</>
		)}
	</svg>
);

export const DiagnosticsBanner = () => {
	const [shown, setShown] = useState<ShownDiagnosis[]>([]);
	const [dismissed, setDismissed] = useState<Set<string>>(new Set());
	const setAppliedDelay = useAudioStore((state) => state.setAppliedDelay);

	useEffect(() => {
		const id = setInterval(() => {
			audioApi
				.getDiagnostics()
				.catch(() => [])
				.then((latest) => setShown((prev) => reconcileDiagnoses(prev, latest)));
		}, POLL_MS);
		return () => clearInterval(id);
	}, []);

	useEffect(() => {
		setDismissed((prev) => rearmDismissed(prev, shown));
	}, [shown]);

	const top = selectTopDiagnosis(
		shown.map((s) => s.diagnosis),
		dismissed,
	);
	if (!top) {
		return null;
	}

	const dismiss = () => setDismissed((prev) => new Set(prev).add(top.code));
	const delaySamples = top.delay_samples;
	const isError = top.severity === "error";

	return (
		<div
			data-testid="diagnostics-banner"
			data-severity={top.severity}
			role={isError ? "alert" : "status"}
			className={`pointer-events-auto flex items-start gap-3 rounded-lg border bg-card py-3 pl-3.5 pr-2 text-sm text-foreground shadow-[0_8px_24px_-8px_rgb(0_0_0/0.45)] ${
				isError ? "border-red-500/60" : "border-amber-500/60"
			}`}
		>
			<span className={isError ? "text-red-500" : "text-amber-500"}>
				<SeverityIcon severity={top.severity} />
			</span>
			<div className="flex min-w-0 flex-1 flex-col gap-2">
				<p className="leading-snug">{top.message}</p>
				{top.action === "apply_delay" && delaySamples !== null && (
					<button
						type="button"
						data-testid="diagnostics-fix"
						className="self-start rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90"
						onClick={() => {
							void applyMeasuredDelay(
								{ delay_samples: delaySamples, is_valid: true },
								setAppliedDelay,
							);
						}}
					>
						Apply {delaySamples} sample delay
					</button>
				)}
			</div>
			<button
				type="button"
				data-testid="diagnostics-dismiss"
				onClick={dismiss}
				aria-label="Dismiss"
				title="Dismiss"
				className="rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
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
					<path d="M18 6 6 18M6 6l12 12" />
				</svg>
			</button>
		</div>
	);
};
