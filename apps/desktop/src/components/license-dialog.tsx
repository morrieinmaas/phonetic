import { useState } from "react";
import { audioApi } from "../services/audio-api";
import type { LicenseState } from "../types/audio";

interface LicenseDialogProps {
	state: LicenseState;
	onStateChange: (state: LicenseState) => void;
}

/** An expired trial blocks the measurement UI; a trial or a licence never does. */
export const shouldBlockUsage = (state: LicenseState): boolean => state.state === "expired";

/** A banner message for a running trial, or null when there is nothing to say. */
export const trialMessage = (state: LicenseState): string | null => {
	if (state.state !== "trial") {
		return null;
	}
	const unit = state.days_remaining === 1 ? "day" : "days";
	return `Trial — ${state.days_remaining} ${unit} left`;
};

/**
 * Renders a blocking activation dialog once the trial has expired, or a
 * dismissible trial banner while it is still running. Renders nothing for a
 * licensed user.
 */
export const LicenseDialog = ({ state, onStateChange }: LicenseDialogProps) => {
	const [key, setKey] = useState("");
	const [error, setError] = useState<string | null>(null);
	const [isActivating, setIsActivating] = useState(false);
	const [dismissed, setDismissed] = useState(false);

	const handleActivate = async () => {
		setError(null);
		setIsActivating(true);
		try {
			const next = await audioApi.activateLicense(key);
			onStateChange(next);
			setKey("");
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		} finally {
			setIsActivating(false);
		}
	};

	if (shouldBlockUsage(state)) {
		return (
			<div className="fixed inset-0 z-50 flex items-center justify-center">
				<div className="absolute inset-0 bg-black/50 backdrop-blur-sm" />
				<div
					data-testid="license-dialog"
					className="relative glass w-full max-w-md mx-4 space-y-4 rounded-lg border border-border p-4 shadow-xl"
				>
					<div>
						<h2 className="text-lg font-semibold">Trial Expired</h2>
						<p className="text-sm text-muted-foreground">
							Your Phonetic trial has ended. Enter a licence key to keep measuring — your stored
							traces and sessions are untouched.
						</p>
					</div>

					<input
						data-testid="license-key-input"
						type="text"
						value={key}
						onChange={(e) => setKey(e.target.value)}
						placeholder="Licence key"
						className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
					/>

					{error && <p className="text-sm text-destructive">{error}</p>}

					<button
						type="button"
						data-testid="license-activate"
						onClick={() => {
							void handleActivate();
						}}
						disabled={isActivating || key.trim() === ""}
						className="w-full rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:cursor-not-allowed disabled:opacity-50"
					>
						{isActivating ? "Activating..." : "Activate"}
					</button>
				</div>
			</div>
		);
	}

	const message = trialMessage(state);
	if (!message || dismissed) {
		return null;
	}

	return (
		<div
			data-testid="trial-banner"
			className="flex items-center gap-3 rounded-lg border border-amber-500/60 bg-card px-4 py-2.5 text-sm text-amber-400 shadow-lg"
		>
			<span className="flex-1">{message}</span>
			<button type="button" onClick={() => setDismissed(true)}>
				Dismiss
			</button>
		</div>
	);
};
