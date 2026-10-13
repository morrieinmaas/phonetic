import type { SignalType } from "../types/audio";

interface GeneratorControlsProps {
	isActive: boolean;
	signalType: SignalType;
	frequency: number;
	amplitude: number;
	onSignalTypeChange: (type: SignalType) => void;
	onFrequencyChange: (frequency: number) => void;
	onAmplitudeChange: (amplitude: number) => void;
	onStart: () => void;
	onStop: () => void;
	disabled?: boolean;
}

const SIGNAL_TYPES: { value: SignalType; label: string }[] = [
	{ value: "sine", label: "Sine" },
	{ value: "white_noise", label: "White Noise" },
	{ value: "pink_noise", label: "Pink Noise" },
	{ value: "sweep", label: "Log Sweep" },
];

const formatFrequency = (freq: number): string => {
	if (freq >= 1000) {
		return `${(freq / 1000).toFixed(1)}kHz`;
	}
	return `${freq}Hz`;
};

export const GeneratorControls = ({
	isActive,
	signalType,
	frequency,
	amplitude,
	onSignalTypeChange,
	onFrequencyChange,
	onAmplitudeChange,
	onStart,
	onStop,
	disabled = false,
}: GeneratorControlsProps) => {
	// Logarithmic frequency slider (20Hz - 20kHz)
	const freqToSlider = (freq: number): number => {
		const minLog = Math.log10(20);
		const maxLog = Math.log10(20000);
		return ((Math.log10(freq) - minLog) / (maxLog - minLog)) * 100;
	};

	const sliderToFreq = (slider: number): number => {
		const minLog = Math.log10(20);
		const maxLog = Math.log10(20000);
		const logValue = minLog + (slider / 100) * (maxLog - minLog);
		return Math.round(10 ** logValue);
	};

	const handleFrequencySlider = (e: React.ChangeEvent<HTMLInputElement>) => {
		const freq = sliderToFreq(Number.parseFloat(e.target.value));
		onFrequencyChange(freq);
	};

	const handleAmplitudeSlider = (e: React.ChangeEvent<HTMLInputElement>) => {
		onAmplitudeChange(Number.parseFloat(e.target.value));
	};

	const showFrequencyControl = signalType === "sine";

	return (
		<div className="flex flex-col gap-4">
			<h3 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
				Signal Generator
			</h3>

			{/* Signal type buttons */}
			<div data-testid="generator-signal-type" className="grid grid-cols-2 gap-2">
				{SIGNAL_TYPES.map(({ value, label }) => (
					<button
						type="button"
						key={value}
						onClick={() => onSignalTypeChange(value)}
						disabled={disabled}
						className={`rounded-md px-3 py-2 text-xs font-medium transition-colors ${
							signalType === value
								? "bg-primary text-primary-foreground"
								: "glass border border-border hover:bg-muted"
						} disabled:cursor-not-allowed disabled:opacity-50`}
					>
						{label}
					</button>
				))}
			</div>

			{/* Frequency control (only for sine) */}
			{showFrequencyControl && (
				<div className="flex flex-col gap-1.5">
					<div className="flex items-center justify-between">
						<label
							htmlFor="generator-frequency"
							className="text-xs font-medium uppercase tracking-wider text-muted-foreground"
						>
							Frequency
						</label>
						<span className="text-xs font-mono text-foreground">{formatFrequency(frequency)}</span>
					</div>
					<input
						id="generator-frequency"
						type="range"
						min="0"
						max="100"
						step="0.1"
						value={freqToSlider(frequency)}
						onChange={handleFrequencySlider}
						disabled={disabled}
						className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-border accent-primary disabled:cursor-not-allowed disabled:opacity-50"
					/>
					<div className="flex justify-between text-[10px] text-muted-foreground">
						<span>20Hz</span>
						<span>1kHz</span>
						<span>20kHz</span>
					</div>
				</div>
			)}

			{/* Amplitude control */}
			<div className="flex flex-col gap-1.5">
				<div className="flex items-center justify-between">
					<label
						htmlFor="generator-amplitude"
						className="text-xs font-medium uppercase tracking-wider text-muted-foreground"
					>
						Amplitude
					</label>
					<span className="text-xs font-mono text-foreground">{(amplitude * 100).toFixed(0)}%</span>
				</div>
				<input
					id="generator-amplitude"
					type="range"
					min="0"
					max="1"
					step="0.01"
					value={amplitude}
					onChange={handleAmplitudeSlider}
					disabled={disabled}
					className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-border accent-primary disabled:cursor-not-allowed disabled:opacity-50"
				/>
			</div>

			{/* Start/Stop button */}
			<button
				type="button"
				data-testid="generator-toggle"
				onClick={isActive ? onStop : onStart}
				disabled={disabled}
				className={`w-full rounded-md py-2.5 text-sm font-semibold transition-colors ${
					isActive
						? "bg-red-500 text-white hover:bg-red-600"
						: "bg-primary text-primary-foreground hover:bg-primary/90"
				} disabled:cursor-not-allowed disabled:opacity-50`}
			>
				{isActive ? "Stop Generator" : "Start Generator"}
			</button>
		</div>
	);
};
