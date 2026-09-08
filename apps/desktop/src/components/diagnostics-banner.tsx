import { useEffect, useState } from "react";
import { audioApi } from "../services/audio-api";
import { applyMeasuredDelay } from "../services/delay-apply";
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

const POLL_MS = 1000;

export const DiagnosticsBanner = () => {
	const [diagnoses, setDiagnoses] = useState<Diagnosis[]>([]);
	const [dismissed, setDismissed] = useState<Set<string>>(new Set());

	useEffect(() => {
		const id = setInterval(() => {
			audioApi
				.getDiagnostics()
				.then(setDiagnoses)
				.catch(() => setDiagnoses([]));
		}, POLL_MS);
		return () => clearInterval(id);
	}, []);

	const top = selectTopDiagnosis(diagnoses, dismissed);
	if (!top) {
		return null;
	}

	const dismiss = () => setDismissed((prev) => new Set(prev).add(top.code));
	const delaySamples = top.delay_samples;

	return (
		<div
			data-testid="diagnostics-banner"
			data-severity={top.severity}
			className={
				top.severity === "error"
					? "flex items-start gap-3 rounded-lg border border-red-500/60 bg-card px-4 py-2.5 text-sm text-red-400 shadow-lg"
					: "flex items-start gap-3 rounded-lg border border-amber-500/60 bg-card px-4 py-2.5 text-sm text-amber-400 shadow-lg"
			}
		>
			<span className="flex-1">{top.message}</span>
			{top.action === "apply_delay" && delaySamples !== null && (
				<button
					type="button"
					data-testid="diagnostics-fix"
					className="rounded-md border px-2 py-1"
					onClick={() => {
						void applyMeasuredDelay({ delay_samples: delaySamples, is_valid: true });
					}}
				>
					Apply
				</button>
			)}
			<button type="button" data-testid="diagnostics-dismiss" onClick={dismiss}>
				Dismiss
			</button>
		</div>
	);
};
