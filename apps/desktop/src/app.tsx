import { useEffect, useRef, useState } from "react";
import { DeviceSelector } from "./components/device-selector";
import { GeneratorControls } from "./components/generator-controls";
import { SpectrumAnalyzer } from "./components/spectrum-analyzer";
import { StatusBar } from "./components/status-bar";
import { TransferFunctionDisplay } from "./components/transfer-function-display";
import { ThemeToggle } from "./components/ui/theme-toggle";
import { WebGLSpectrum } from "./components/webgl-spectrum";
import { useAudioStore } from "./stores/audio-store";

type SpectrumRenderer = "uplot" | "webgl";

export const App = () => {
	const [theme, setTheme] = useState<"dark" | "light">(() => {
		if (typeof window !== "undefined") {
			const stored = localStorage.getItem("phonetic-theme");
			if (stored === "light" || stored === "dark") return stored;
			return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
		}
		return "dark";
	});

	const [spectrumRenderer, setSpectrumRenderer] = useState<SpectrumRenderer>(() => {
		if (typeof window !== "undefined") {
			const stored = localStorage.getItem("phonetic-spectrum-renderer");
			if (stored === "uplot" || stored === "webgl") return stored;
		}
		return "uplot";
	});

	// Animation frame ref for spectrum polling
	const animationFrameRef = useRef<number | null>(null);

	// Get store state and actions
	const {
		devices,
		selectedInputDevice,
		selectedOutputDevice,
		audioState,
		isLoading,
		error,
		generatorFrequency,
		generatorAmplitude,
		generatorType,
		spectrumData,
		transferFunctionData,
		fftSize,
		fetchDevices,
		fetchAudioState,
		setSelectedInputDevice,
		setSelectedOutputDevice,
		startStream,
		stopStream,
		startGenerator,
		stopGenerator,
		setGeneratorFrequency,
		setGeneratorAmplitude,
		setGeneratorType,
		fetchSpectrum,
		fetchTransferFunction,
		resetAveraging,
		clearError,
	} = useAudioStore();

	// Initialize on mount
	useEffect(() => {
		fetchDevices();
		fetchAudioState();
	}, [fetchDevices, fetchAudioState]);

	// Apply theme
	useEffect(() => {
		document.documentElement.classList.remove("light", "dark");
		document.documentElement.classList.add(theme);
		localStorage.setItem("phonetic-theme", theme);
	}, [theme]);

	// Persist spectrum renderer preference
	useEffect(() => {
		localStorage.setItem("phonetic-spectrum-renderer", spectrumRenderer);
	}, [spectrumRenderer]);

	// Spectrum polling loop
	useEffect(() => {
		const isRunning = audioState?.is_running ?? false;

		const pollSpectrumData = () => {
			if (isRunning) {
				fetchSpectrum();
				fetchTransferFunction();
			}
			animationFrameRef.current = requestAnimationFrame(pollSpectrumData);
		};

		if (isRunning) {
			animationFrameRef.current = requestAnimationFrame(pollSpectrumData);
		}

		return () => {
			if (animationFrameRef.current !== null) {
				cancelAnimationFrame(animationFrameRef.current);
				animationFrameRef.current = null;
			}
		};
	}, [audioState?.is_running, fetchSpectrum, fetchTransferFunction]);

	const toggleTheme = () => {
		setTheme((prev) => (prev === "dark" ? "light" : "dark"));
	};

	const handleStartStream = async () => {
		await startStream();
	};

	const handleStopStream = async () => {
		await stopStream();
	};

	const isRunning = audioState?.is_running ?? false;
	const generatorActive = audioState?.generator_active ?? false;

	return (
		<div className="flex min-h-screen flex-col bg-background text-foreground">
			{/* Header */}
			<header className="glass sticky top-0 z-50 flex h-14 items-center justify-between border-b px-4">
				<h1 className="gradient-text text-xl font-bold tracking-tight">Phonetic</h1>
				<ThemeToggle theme={theme} onToggle={toggleTheme} />
			</header>

			{/* Main content */}
			<main className="flex flex-1 gap-4 p-4 min-h-0">
				{/* Sidebar - Device & Generator controls */}
				<aside className="glass w-72 flex-shrink-0 rounded-lg p-4 flex flex-col gap-6 overflow-y-auto">
					{/* Audio Devices Section */}
					<div className="flex flex-col gap-4">
						<h2 className="text-sm font-semibold uppercase tracking-wider text-muted-foreground">
							Audio Devices
						</h2>

						{devices ? (
							<>
								<DeviceSelector
									label="Input"
									devices={devices.inputs}
									selectedDeviceId={selectedInputDevice}
									defaultDeviceId={devices.default_input}
									onSelect={setSelectedInputDevice}
									disabled={isRunning}
								/>

								<DeviceSelector
									label="Output"
									devices={devices.outputs}
									selectedDeviceId={selectedOutputDevice}
									defaultDeviceId={devices.default_output}
									onSelect={setSelectedOutputDevice}
									disabled={isRunning}
								/>

								<button
									type="button"
									onClick={isRunning ? handleStopStream : handleStartStream}
									disabled={isLoading}
									className={`w-full rounded-md py-2.5 text-sm font-semibold transition-colors ${
										isRunning
											? "bg-red-500 text-white hover:bg-red-600"
											: "bg-primary text-primary-foreground hover:bg-primary/90"
									} disabled:cursor-not-allowed disabled:opacity-50`}
								>
									{isLoading ? "..." : isRunning ? "Stop Stream" : "Start Stream"}
								</button>
							</>
						) : (
							<p className="text-sm text-muted-foreground">Loading devices...</p>
						)}
					</div>

					{/* Divider */}
					<div className="border-t border-border" />

					{/* Generator Controls */}
					<GeneratorControls
						isActive={generatorActive}
						signalType={generatorType}
						frequency={generatorFrequency}
						amplitude={generatorAmplitude}
						onSignalTypeChange={setGeneratorType}
						onFrequencyChange={setGeneratorFrequency}
						onAmplitudeChange={setGeneratorAmplitude}
						onStart={startGenerator}
						onStop={stopGenerator}
						disabled={!isRunning}
					/>

					{/* Error display */}
					{error && (
						<div className="rounded-md bg-red-500/10 border border-red-500/20 p-3 text-sm text-red-400">
							<div className="flex justify-between items-start">
								<span>{error}</span>
								<button
									type="button"
									onClick={clearError}
									className="text-red-400 hover:text-red-300"
								>
									×
								</button>
							</div>
						</div>
					)}
				</aside>

				{/* Main visualization area */}
				<div className="flex flex-1 flex-col gap-4 min-h-0">
					{/* Spectrum analyzer - takes slightly more space */}
					<div className="glass rounded-lg p-4 flex-[1.2] min-h-[350px] flex flex-col">
						{/* Renderer toggle */}
						<div className="flex items-center justify-end gap-2 mb-2 flex-shrink-0">
							<span className="text-xs text-muted-foreground">Renderer:</span>
							<div className="flex rounded-md border border-border overflow-hidden">
								<button
									type="button"
									onClick={() => setSpectrumRenderer("uplot")}
									className={`px-2 py-1 text-xs transition-colors ${
										spectrumRenderer === "uplot"
											? "bg-primary text-primary-foreground"
											: "bg-transparent text-muted-foreground hover:bg-muted"
									}`}
								>
									uPlot
								</button>
								<button
									type="button"
									onClick={() => setSpectrumRenderer("webgl")}
									className={`px-2 py-1 text-xs transition-colors ${
										spectrumRenderer === "webgl"
											? "bg-primary text-primary-foreground"
											: "bg-transparent text-muted-foreground hover:bg-muted"
									}`}
								>
									WebGL
								</button>
							</div>
						</div>
						{/* Spectrum visualizer */}
						<div className="flex-1 min-h-0">
							{spectrumRenderer === "webgl" ? (
								<WebGLSpectrum
									spectrumData={spectrumData}
									sampleRate={audioState?.sample_rate ?? 48000}
									fftSize={fftSize}
									isRunning={isRunning}
								/>
							) : (
								<SpectrumAnalyzer
									spectrumData={spectrumData}
									sampleRate={audioState?.sample_rate ?? 48000}
									fftSize={fftSize}
									isRunning={isRunning}
								/>
							)}
						</div>
					</div>

					{/* Transfer function display */}
					<div className="glass rounded-lg p-4 flex-1 min-h-[300px]">
						<TransferFunctionDisplay
							data={transferFunctionData}
							isRunning={isRunning}
							onResetAveraging={resetAveraging}
						/>
					</div>
				</div>
			</main>

			{/* Footer status bar */}
			<StatusBar audioState={audioState} error={error} />
		</div>
	);
};
