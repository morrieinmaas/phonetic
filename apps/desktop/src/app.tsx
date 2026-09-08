import { useEffect, useState } from "react";
import { ChannelSelector } from "./components/channel-selector";
import { DeviceSelector } from "./components/device-selector";
import { DiagnosticsBanner } from "./components/diagnostics-banner";
import { GeneratorControls } from "./components/generator-controls";
import { LicenseDialog, shouldBlockUsage } from "./components/license-dialog";
import { PermissionDialog } from "./components/permission-dialog";
import { SessionControls } from "./components/session-controls";
import { SettingsDialog } from "./components/settings-dialog";
import { SpectrogramDisplay } from "./components/spectrogram-display";
import { SpectrumAnalyzer } from "./components/spectrum-analyzer";
import { StatusBar } from "./components/status-bar";
import { TraceControls } from "./components/trace-controls";
import { TransferFunctionDisplay } from "./components/transfer-function-display";
import { ThemeToggle } from "./components/ui/theme-toggle";
import { WebGLSpectrum } from "./components/webgl-spectrum";
import { createPollLoop } from "./lib/poll-loop";
import { audioApi } from "./services/audio-api";
import { useAudioStore } from "./stores/audio-store";
import type { LicenseState } from "./types/audio";

type SpectrumRenderer = "uplot" | "webgl";
type SecondaryView = "transfer" | "spectrogram";

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

	const [secondaryView, setSecondaryView] = useState<SecondaryView>(() => {
		if (typeof window !== "undefined") {
			const stored = localStorage.getItem("phonetic-secondary-view");
			if (stored === "transfer" || stored === "spectrogram") return stored;
		}
		return "spectrogram";
	});

	const [settingsOpen, setSettingsOpen] = useState(false);
	const [licenseState, setLicenseState] = useState<LicenseState | null>(null);
	const [permissionDialogOpen, setPermissionDialogOpen] = useState(false);

	// Animation frame ref for spectrum polling

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
		inputChannelCount,
		measurementChannel,
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
		setFftSize,
		setInputChannelCount,
		setMeasurementChannel,
	} = useAudioStore();

	const selectedInputAudioDevice =
		devices?.inputs.find((device) => device.id === selectedInputDevice) ?? null;

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

	// Persist secondary view preference
	useEffect(() => {
		localStorage.setItem("phonetic-secondary-view", secondaryView);
	}, [secondaryView]);

	useEffect(() => {
		audioApi
			.getLicenseState()
			.then(setLicenseState)
			.catch(() => setLicenseState(null));
	}, []);

	// Spectrum polling loop.
	//
	// Each poll must finish before the next is scheduled. Firing these without
	// awaiting them queues a fresh pair of IPC calls every frame; once the
	// backend takes longer than the frame budget — which it does as soon as the
	// generator starts and the audio callback begins real FFT work under the
	// same lock — the queue grows without bound and the window freezes.
	useEffect(() => {
		const isRunning = audioState?.is_running ?? false;
		if (!isRunning) {
			return;
		}

		return createPollLoop(
			async () => {
				await Promise.all([fetchSpectrum(), fetchTransferFunction()]);
			},
			(callback) => requestAnimationFrame(callback),
			(handle) => cancelAnimationFrame(handle),
		);
	}, [audioState?.is_running, fetchSpectrum, fetchTransferFunction]);

	const toggleTheme = () => {
		setTheme((prev) => (prev === "dark" ? "light" : "dark"));
	};

	const handleStartStream = async () => {
		await startStream();
		// Check if there was a permission error after attempting to start
		const currentError = useAudioStore.getState().error;
		if (currentError && isPermissionError(currentError)) {
			setPermissionDialogOpen(true);
		}
	};

	// Detect permission-related errors
	const isPermissionError = (errorMessage: string): boolean => {
		const permissionKeywords = [
			"permission",
			"denied",
			"not authorized",
			"access denied",
			"microphone",
			"input device",
			"no input",
			"cannot access",
		];
		const lowerError = errorMessage.toLowerCase();
		return permissionKeywords.some((keyword) => lowerError.includes(keyword));
	};

	const handleRetryWithPermission = async () => {
		setPermissionDialogOpen(false);
		clearError();
		// Small delay to allow user to grant permission
		await new Promise((resolve) => setTimeout(resolve, 500));
		await startStream();
	};

	const handleStopStream = async () => {
		await stopStream();
	};

	const isRunning = audioState?.is_running ?? false;
	const generatorActive = audioState?.generator_active ?? false;

	return (
		<div className="flex h-screen overflow-hidden flex-col bg-background text-foreground">
			{/* Header */}
			<header className="glass sticky top-0 z-50 flex h-14 items-center justify-between border-b px-4">
				<h1 className="gradient-text text-xl font-bold tracking-tight">Phonetic</h1>
				<div className="flex items-center gap-2">
					<SessionControls />
					<button
						type="button"
						data-testid="settings-open"
						onClick={() => setSettingsOpen(true)}
						className="p-2 rounded-md hover:bg-muted transition-colors"
						aria-label="Open settings"
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
							<title>Settings</title>
							<path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" />
							<circle cx="12" cy="12" r="3" />
						</svg>
					</button>
					<ThemeToggle theme={theme} onToggle={toggleTheme} />
				</div>
			</header>

			{/* Rig diagnoses sit above the measurement area: they appear and vanish
			    on a poll, and below the status bar they shoved the footer around. */}
			<div className="flex-shrink-0 px-4 pt-4 empty:hidden">
				{licenseState && !shouldBlockUsage(licenseState) && (
					<LicenseDialog state={licenseState} onStateChange={setLicenseState} />
				)}
				<DiagnosticsBanner />
			</div>

			{/* Main content */}
			{licenseState && shouldBlockUsage(licenseState) ? (
				<div className="flex flex-1 items-center justify-center p-4">
					<LicenseDialog state={licenseState} onStateChange={setLicenseState} />
				</div>
			) : (
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
										testId="input-device-select"
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

									<ChannelSelector
										inputDevice={selectedInputAudioDevice}
										channelCount={inputChannelCount}
										measurementChannel={measurementChannel}
										onChannelCountChange={setInputChannelCount}
										onMeasurementChannelChange={setMeasurementChannel}
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

						{/* Trace capture, A/B compare, delay apply and spatial averaging */}
						<TraceControls currentData={transferFunctionData} isRunning={isRunning} />

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

						{/* Secondary view - Transfer function or Spectrogram */}
						<div className="glass rounded-lg p-4 flex-1 min-h-[300px] flex flex-col">
							{/* View toggle */}
							<div className="flex items-center justify-end gap-2 mb-2 flex-shrink-0">
								<span className="text-xs text-muted-foreground">View:</span>
								<div className="flex rounded-md border border-border overflow-hidden">
									<button
										type="button"
										onClick={() => setSecondaryView("transfer")}
										className={`px-2 py-1 text-xs transition-colors ${
											secondaryView === "transfer"
												? "bg-primary text-primary-foreground"
												: "bg-transparent text-muted-foreground hover:bg-muted"
										}`}
									>
										Transfer Function
									</button>
									<button
										type="button"
										onClick={() => setSecondaryView("spectrogram")}
										className={`px-2 py-1 text-xs transition-colors ${
											secondaryView === "spectrogram"
												? "bg-primary text-primary-foreground"
												: "bg-transparent text-muted-foreground hover:bg-muted"
										}`}
									>
										Spectrogram
									</button>
								</div>
							</div>
							{/* View content */}
							<div className="flex-1 min-h-0">
								{secondaryView === "transfer" ? (
									<TransferFunctionDisplay
										data={transferFunctionData}
										isRunning={isRunning}
										onResetAveraging={resetAveraging}
										sampleRate={audioState?.sample_rate ?? 48000}
									/>
								) : (
									<SpectrogramDisplay
										spectrumData={spectrumData}
										sampleRate={audioState?.sample_rate ?? 48000}
										fftSize={fftSize}
										isRunning={isRunning}
									/>
								)}
							</div>
						</div>
					</div>
				</main>
			)}

			{/* Footer status bar */}
			<StatusBar audioState={audioState} error={error} />

			{/* Settings dialog */}
			<SettingsDialog
				isOpen={settingsOpen}
				onClose={() => setSettingsOpen(false)}
				audioState={audioState}
				currentFftSize={fftSize}
				onFftSizeChange={setFftSize}
			/>

			{/* Permission dialog */}
			<PermissionDialog
				isOpen={permissionDialogOpen}
				onClose={() => setPermissionDialogOpen(false)}
				onRetry={handleRetryWithPermission}
			/>
		</div>
	);
};
