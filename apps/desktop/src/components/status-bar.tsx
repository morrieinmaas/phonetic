import type { AudioState } from "../types/audio";

interface StatusBarProps {
	audioState: AudioState | null;
	error: string | null;
}

const formatSampleRate = (rate: number): string => {
	if (rate >= 1000) {
		return `${(rate / 1000).toFixed(1)} kHz`;
	}
	return `${rate} Hz`;
};

export const StatusBar = ({ audioState, error }: StatusBarProps) => {
	const isRunning = audioState?.is_running ?? false;
	const sampleRate = audioState?.sample_rate ?? 0;
	const bufferSize = audioState?.buffer_size ?? 0;
	const generatorActive = audioState?.generator_active ?? false;
	const generatorType = audioState?.generator_type ?? null;

	// Calculate approximate latency in ms: (buffer_size / sample_rate) * 1000
	const latency = sampleRate > 0 ? ((bufferSize / sampleRate) * 1000).toFixed(1) : "--";

	return (
		<footer className="glass flex h-8 items-center justify-between border-t px-4 text-xs">
			{/* Left section - Status indicators */}
			<div className="flex items-center gap-4">
				<div className="flex items-center gap-1.5">
					<span
						className={`h-2 w-2 rounded-full ${isRunning ? "bg-green-500" : "bg-muted-foreground"}`}
					/>
					<span className="text-muted-foreground">{isRunning ? "Running" : "Stopped"}</span>
				</div>

				{generatorActive && generatorType && (
					<div className="flex items-center gap-1.5">
						<span className="h-2 w-2 rounded-full bg-primary animate-pulse" />
						<span className="text-muted-foreground capitalize">
							Gen: {generatorType.replace("_", " ")}
						</span>
					</div>
				)}
			</div>

			{/* Center section - Error display */}
			{error && (
				<div className="text-red-400 truncate max-w-md" title={error}>
					{error}
				</div>
			)}

			{/* Right section - Audio info */}
			<div className="flex items-center gap-4 text-muted-foreground">
				<span>
					Sample Rate:{" "}
					<span className="font-mono">{sampleRate > 0 ? formatSampleRate(sampleRate) : "--"}</span>
				</span>
				<span>
					Buffer:{" "}
					<span className="font-mono">{bufferSize > 0 ? `${bufferSize} samples` : "--"}</span>
				</span>
				<span>
					Latency: <span className="font-mono">{latency} ms</span>
				</span>
			</div>
		</footer>
	);
};
