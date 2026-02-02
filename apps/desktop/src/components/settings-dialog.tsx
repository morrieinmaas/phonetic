import { useEffect, useState } from "react";
import { audioApi, type AudioState } from "../services/audio-api";

interface SettingsDialogProps {
	isOpen: boolean;
	onClose: () => void;
	audioState: AudioState | null;
	currentFftSize: number;
	onFftSizeChange: (size: number) => Promise<void>;
}

const FFT_SIZES = [512, 1024, 2048, 4096, 8192, 16384];

export const SettingsDialog = ({
	isOpen,
	onClose,
	audioState,
	currentFftSize,
	onFftSizeChange,
}: SettingsDialogProps) => {
	const [selectedBackend, setSelectedBackend] = useState<string>("cpal");
	const [selectedFftSize, setSelectedFftSize] = useState<number>(currentFftSize);
	const [isSaving, setIsSaving] = useState(false);
	const [error, setError] = useState<string | null>(null);

	useEffect(() => {
		if (audioState) {
			setSelectedBackend(audioState.backend);
		}
		setSelectedFftSize(currentFftSize);
	}, [audioState, currentFftSize, isOpen]);

	if (!isOpen) return null;

	const handleBackendChange = async (backend: string) => {
		setSelectedBackend(backend);
		setError(null);
		try {
			await audioApi.setAudioBackend(backend as "cpal" | "coreaudio");
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		}
	};

	const handleFftSizeChange = async (size: number) => {
		setSelectedFftSize(size);
		setIsSaving(true);
		setError(null);
		try {
			await onFftSizeChange(size);
		} catch (e) {
			setError(e instanceof Error ? e.message : String(e));
		} finally {
			setIsSaving(false);
		}
	};

	const availableBackends = audioState?.backend_available ?? ["cpal"];

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center">
			{/* Backdrop */}
			<div className="absolute inset-0 bg-black/50 backdrop-blur-sm" onClick={onClose} />

			{/* Dialog */}
			<div className="relative glass rounded-lg border border-border shadow-xl w-full max-w-md mx-4">
				{/* Header */}
				<div className="flex items-center justify-between p-4 border-b border-border">
					<h2 className="text-lg font-semibold">Settings</h2>
					<button
						type="button"
						onClick={onClose}
						className="p-1 rounded hover:bg-muted transition-colors"
						aria-label="Close settings"
					>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							width="20"
							height="20"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							strokeWidth="2"
							strokeLinecap="round"
							strokeLinejoin="round"
						>
							<title>Close</title>
							<line x1="18" y1="6" x2="6" y2="18" />
							<line x1="6" y1="6" x2="18" y2="18" />
						</svg>
					</button>
				</div>

				{/* Content */}
				<div className="p-4 space-y-6">
					{/* Audio Backend */}
					<div className="space-y-2">
						<label className="text-sm font-medium text-foreground">Audio Backend</label>
						<p className="text-xs text-muted-foreground mb-2">
							{selectedBackend === "coreaudio"
								? "CoreAudio: Lower latency (~2ms), macOS only"
								: "CPAL: Cross-platform, higher latency (~10ms)"}
						</p>
						<div className="flex gap-2">
							{availableBackends.map((backend) => (
								<button
									type="button"
									key={backend}
									onClick={() => handleBackendChange(backend)}
									disabled={audioState?.is_running}
									className={`flex-1 px-3 py-2 text-sm rounded-md border transition-colors ${
										selectedBackend === backend
											? "bg-primary text-primary-foreground border-primary"
											: "border-border hover:bg-muted"
									} disabled:opacity-50 disabled:cursor-not-allowed`}
								>
									{backend === "coreaudio" ? "CoreAudio" : "CPAL"}
								</button>
							))}
						</div>
						{audioState?.is_running && (
							<p className="text-xs text-yellow-500">Stop audio stream to change backend</p>
						)}
					</div>

					{/* FFT Size */}
					<div className="space-y-2">
						<label className="text-sm font-medium text-foreground">FFT Size</label>
						<p className="text-xs text-muted-foreground mb-2">
							Larger = better frequency resolution, slower updates. Current: {selectedFftSize} bins (
							{audioState ? ((audioState.sample_rate / selectedFftSize) * 1000).toFixed(1) : "?"} ms
							window)
						</p>
						<div className="grid grid-cols-3 gap-2">
							{FFT_SIZES.map((size) => (
								<button
									type="button"
									key={size}
									onClick={() => handleFftSizeChange(size)}
									disabled={isSaving}
									className={`px-3 py-2 text-sm rounded-md border transition-colors ${
										selectedFftSize === size
											? "bg-primary text-primary-foreground border-primary"
											: "border-border hover:bg-muted"
									} disabled:opacity-50`}
								>
									{size}
								</button>
							))}
						</div>
					</div>

					{/* Audio Info */}
					{audioState && (
						<div className="space-y-2">
							<label className="text-sm font-medium text-foreground">Current Audio State</label>
							<div className="text-xs space-y-1 p-3 rounded-md bg-muted/50">
								<div className="flex justify-between">
									<span className="text-muted-foreground">Sample Rate:</span>
									<span className="font-mono">{audioState.sample_rate.toLocaleString()} Hz</span>
								</div>
								<div className="flex justify-between">
									<span className="text-muted-foreground">Buffer Size:</span>
									<span className="font-mono">{audioState.buffer_size} samples</span>
								</div>
								<div className="flex justify-between">
									<span className="text-muted-foreground">Latency:</span>
									<span className="font-mono">
										{((audioState.buffer_size / audioState.sample_rate) * 1000).toFixed(1)} ms
									</span>
								</div>
								<div className="flex justify-between">
									<span className="text-muted-foreground">Frequency Resolution:</span>
									<span className="font-mono">
										{(audioState.sample_rate / selectedFftSize).toFixed(2)} Hz/bin
									</span>
								</div>
							</div>
						</div>
					)}

					{/* Error display */}
					{error && (
						<div className="p-3 rounded-md bg-red-500/10 border border-red-500/20 text-red-500 text-sm">
							{error}
						</div>
					)}
				</div>

				{/* Footer */}
				<div className="flex justify-end p-4 border-t border-border">
					<button
						type="button"
						onClick={onClose}
						className="px-4 py-2 text-sm font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
					>
						Done
					</button>
				</div>
			</div>
		</div>
	);
};
