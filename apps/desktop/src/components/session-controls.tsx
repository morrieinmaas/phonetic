import { open, save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";
import { audioApi } from "../services/audio-api";
import { useAudioStore } from "../stores/audio-store";
import { useTraceStore } from "../stores/trace-store";
import type { SessionSettings } from "../types/audio";

const SESSION_FILTERS = [{ name: "Phonetic session", extensions: ["json"] }];
const DEFAULT_SESSION_NAME = "Untitled session";

/** Settings not yet exposed as their own UI controls are captured at their current fixed default. */
const buildSessionSettings = (sampleRate: number, fftSize: number): SessionSettings => ({
	sample_rate: sampleRate,
	fft_size: fftSize,
	multi_window: false,
	smoothing: "none",
	delay_samples: 0,
	max_averages: 16,
});

export const SessionControls = () => {
	const [error, setError] = useState<string | null>(null);
	const sampleRate = useAudioStore((state) => state.audioState?.sample_rate ?? 48000);
	const fftSize = useAudioStore((state) => state.fftSize);

	const handleSave = async () => {
		setError(null);
		try {
			const path = await save({ filters: SESSION_FILTERS });
			if (!path) return;
			const settings = buildSessionSettings(sampleRate, fftSize);
			const session = useTraceStore.getState().toSession(DEFAULT_SESSION_NAME, settings);
			await audioApi.saveSession(path, session);
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		}
	};

	const handleOpen = async () => {
		setError(null);
		try {
			const path = await open({ filters: SESSION_FILTERS });
			if (!path) return;
			const session = await audioApi.loadSession(path);
			useTraceStore.getState().fromSession(session);
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		}
	};

	const handleNew = () => {
		setError(null);
		useTraceStore.getState().clearAllTraces();
	};

	return (
		<div className="flex items-center gap-2">
			<div className="flex items-center gap-1">
				<button
					type="button"
					data-testid="session-save"
					onClick={handleSave}
					title="Save the current session to a file"
					className="px-2 py-1 text-xs rounded border border-border hover:bg-muted transition-colors"
				>
					Save
				</button>
				<button
					type="button"
					data-testid="session-open"
					onClick={handleOpen}
					title="Open a session from a file"
					className="px-2 py-1 text-xs rounded border border-border hover:bg-muted transition-colors"
				>
					Open
				</button>
				<button
					type="button"
					data-testid="session-new"
					onClick={handleNew}
					title="Start a new session, clearing all stored traces"
					className="px-2 py-1 text-xs rounded border border-border hover:bg-muted transition-colors"
				>
					New
				</button>
			</div>
			{error && (
				<div className="px-2 py-1 rounded-md bg-red-500/10 border border-red-500/20 text-red-500 text-xs">
					{error}
				</div>
			)}
		</div>
	);
};
