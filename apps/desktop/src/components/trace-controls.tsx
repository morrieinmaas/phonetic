import { useCallback, useEffect, useState } from "react";
import { audioApi } from "../services/audio-api";
import { applyMeasuredDelay } from "../services/delay-apply";
import { useAudioStore } from "../stores/audio-store";
import { useTraceStore } from "../stores/trace-store";
import type { TraceSlot, TransferFunctionData } from "../types/audio";

interface TraceControlsProps {
	currentData: TransferFunctionData | null;
	isRunning: boolean;
}

const SLOTS: TraceSlot[] = ["A", "B", "C", "D"];

export const TraceControls = ({ currentData, isRunning }: TraceControlsProps) => {
	const {
		traces,
		slots,
		captureToSlot,
		toggleTraceVisibility,
		deleteTrace,
		clearAllTraces,
		captureTrace,
	} = useTraceStore();
	const [showList, setShowList] = useState(false);
	const [delayInput, setDelayInput] = useState("");
	const [delayRejected, setDelayRejected] = useState(false);
	const appliedDelay = useAudioStore((state) => state.appliedDelaySamples);
	const setAppliedDelay = useAudioStore((state) => state.setAppliedDelay);
	const fetchAppliedDelay = useAudioStore((state) => state.fetchAppliedDelay);
	const sampleRate = useAudioStore((state) => state.audioState?.sample_rate ?? 48000);

	useEffect(() => {
		void fetchAppliedDelay();
	}, [fetchAppliedDelay]);
	const [averageStatus, setAverageStatus] = useState<"idle" | "error">("idle");

	const handleApplyDelay = useCallback(async () => {
		const delaySamples = Number(delayInput);
		const applied = await applyMeasuredDelay(
			{ delay_samples: delaySamples, is_valid: Number.isInteger(delaySamples) },
			setAppliedDelay,
		);
		setDelayRejected(!applied);
		if (applied) setDelayInput("");
	}, [delayInput, setAppliedDelay]);

	const visibleTraces = traces.filter((trace) => trace.visible);

	const handleAverageTraces = useCallback(async () => {
		if (visibleTraces.length === 0) return;
		try {
			const averaged = await audioApi.averageTraces(visibleTraces.map((trace) => trace.data));
			captureTrace(averaged, `Spatial average (${visibleTraces.length})`);
			setAverageStatus("idle");
		} catch {
			setAverageStatus("error");
		}
	}, [visibleTraces, captureTrace]);

	const handleSlotClick = useCallback(
		(slot: TraceSlot) => {
			const existingTraceId = slots[slot];
			if (existingTraceId) {
				// Slot has a trace - delete it to clear the slot
				deleteTrace(existingTraceId);
			} else {
				// Slot is empty - capture to it
				if (!currentData || currentData.frequencies.length === 0) return;
				captureToSlot(slot, currentData);
			}
		},
		[currentData, captureToSlot, deleteTrace, slots],
	);

	const getSlotTrace = (slot: TraceSlot) => {
		const traceId = slots[slot];
		return traceId ? traces.find((t) => t.id === traceId) : undefined;
	};

	const formatTimestamp = (ts: number) => {
		const date = new Date(ts);
		return date.toLocaleTimeString(undefined, {
			hour: "2-digit",
			minute: "2-digit",
			second: "2-digit",
		});
	};

	const canCapture = isRunning && currentData && currentData.frequencies.length > 0;

	return (
		<div className="flex items-center gap-2">
			{/* Slot buttons for quick A/B comparison */}
			<div className="flex items-center gap-1">
				<span
					className="text-xs text-muted-foreground mr-1"
					title={
						canCapture
							? "Click a slot to store the live trace; click it again to clear"
							: "Start the stream to capture a trace"
					}
				>
					Capture:
				</span>
				{SLOTS.map((slot) => {
					const trace = getSlotTrace(slot);
					const hasTrace = !!trace;

					return (
						<button
							key={slot}
							type="button"
							data-testid={slot === "A" ? "capture-trace" : undefined}
							onClick={() => handleSlotClick(slot)}
							disabled={!hasTrace && !canCapture}
							title={
								hasTrace ? `Clear Trace ${slot} (click to remove)` : `Capture to Trace ${slot}`
							}
							className={`
                w-7 h-7 text-xs font-bold rounded border transition-all
                ${hasTrace ? "border-transparent ring-2" : "border-border hover:border-primary/50"}
                disabled:opacity-40 disabled:cursor-not-allowed
                hover:scale-105 active:scale-95
              `}
							style={{
								backgroundColor: hasTrace ? trace.color : undefined,
								color: hasTrace ? "white" : undefined,
								boxShadow: hasTrace ? `0 0 8px ${trace.color}40` : undefined,
								["--tw-ring-color" as string]: hasTrace ? trace.color : undefined,
							}}
						>
							{slot}
						</button>
					);
				})}
			</div>

			{/* Delay compensation: what is applied now, a way to remove it, and a way
			    to type a new value. Applying used to be one-way with no undo. */}
			<div className="ml-2 flex items-center gap-1.5 border-l border-border pl-2">
				<label htmlFor="delay-input" className="text-xs text-muted-foreground">
					Delay
				</label>
				{appliedDelay > 0 && (
					<span
						data-testid="applied-delay"
						className="flex items-center gap-1 rounded border border-primary/50 bg-primary/10 py-0.5 pl-2 pr-0.5 text-xs text-foreground"
					>
						<span className="font-mono tabular-nums">
							{appliedDelay} smp · {((appliedDelay * 1000) / sampleRate).toFixed(2)} ms
						</span>
						<button
							type="button"
							data-testid="clear-delay"
							onClick={() => void setAppliedDelay(0)}
							aria-label="Remove delay compensation"
							title="Remove delay compensation"
							className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
						>
							<svg
								aria-hidden="true"
								width="12"
								height="12"
								viewBox="0 0 24 24"
								fill="none"
								stroke="currentColor"
								strokeWidth="2.5"
								strokeLinecap="round"
							>
								<path d="M18 6 6 18M6 6l12 12" />
							</svg>
						</button>
					</span>
				)}
				<input
					id="delay-input"
					type="number"
					min={0}
					step={1}
					value={delayInput}
					onChange={(e) => {
						setDelayInput(e.target.value);
						setDelayRejected(false);
					}}
					onKeyDown={(e) => {
						if (e.key === "Enter" && delayInput !== "") void handleApplyDelay();
					}}
					placeholder={appliedDelay > 0 ? "new" : "samples"}
					aria-invalid={delayRejected}
					className={`w-20 rounded border bg-background px-1.5 py-0.5 text-xs ${
						delayRejected ? "border-red-500" : "border-border"
					}`}
				/>
				<button
					type="button"
					data-testid="apply-delay"
					onClick={handleApplyDelay}
					disabled={delayInput === ""}
					title="Delay the reference by this many samples so it lines up with the measurement"
					className="rounded border border-border px-2 py-1 text-xs hover:bg-muted disabled:cursor-not-allowed disabled:opacity-40"
				>
					Apply
				</button>
				{delayRejected && (
					<span role="alert" className="text-xs text-red-500">
						Whole samples, 0 or more
					</span>
				)}
			</div>

			{/* Average the currently visible traces into one spatially-averaged trace */}
			<div className="flex items-center gap-1 ml-2 border-l border-border pl-2">
				<button
					type="button"
					data-testid="average-traces"
					onClick={handleAverageTraces}
					disabled={visibleTraces.length === 0}
					title="Average the currently visible traces in the complex domain"
					className="px-2 py-1 text-xs rounded border border-border hover:bg-muted disabled:opacity-40 disabled:cursor-not-allowed"
				>
					Average selected
				</button>
				{averageStatus === "error" && (
					<span className="text-[10px] text-destructive">Averaging failed</span>
				)}
			</div>

			{/* Show/hide toggle buttons for each stored trace */}
			{traces.length > 0 && (
				<div className="flex items-center gap-1 ml-2 border-l border-border pl-2">
					<span className="text-xs text-muted-foreground mr-1">Show:</span>
					{traces.slice(-4).map((trace) => (
						<button
							key={trace.id}
							type="button"
							onClick={() => toggleTraceVisibility(trace.id)}
							title={`${trace.visible ? "Hide" : "Show"} ${trace.name}`}
							className={`
                w-6 h-6 text-[10px] font-bold rounded transition-all
                ${trace.visible ? "opacity-100" : "opacity-30"}
                hover:scale-110
              `}
							style={{
								backgroundColor: trace.color,
								color: "white",
							}}
						>
							{trace.name.replace("Trace ", "")}
						</button>
					))}
				</div>
			)}

			{/* Trace list dropdown */}
			{traces.length > 0 && (
				<div className="relative">
					<button
						type="button"
						onClick={() => setShowList(!showList)}
						className="px-2 py-1 text-xs rounded border border-border hover:bg-muted"
					>
						{traces.length} trace{traces.length !== 1 ? "s" : ""}
					</button>

					{showList && (
						<div className="absolute top-full right-0 mt-1 w-64 bg-popover border border-border rounded-md shadow-lg z-50">
							<div className="p-2 border-b border-border flex items-center justify-between">
								<span className="text-xs font-medium">Stored Traces</span>
								<button
									type="button"
									onClick={clearAllTraces}
									className="text-xs text-destructive hover:underline"
								>
									Clear All
								</button>
							</div>
							<div data-testid="trace-list" className="max-h-48 overflow-y-auto">
								{traces.map((trace) => (
									<div
										key={trace.id}
										className="flex items-center gap-2 p-2 hover:bg-muted/60 group"
									>
										<button
											type="button"
											onClick={() => toggleTraceVisibility(trace.id)}
											className="w-4 h-4 rounded-sm border"
											style={{
												backgroundColor: trace.visible ? trace.color : "transparent",
												borderColor: trace.color,
											}}
										/>
										<span className="flex-1 text-xs truncate">{trace.name}</span>
										<span className="text-[10px] text-muted-foreground">
											{formatTimestamp(trace.timestamp)}
										</span>
										<button
											type="button"
											onClick={() => deleteTrace(trace.id)}
											aria-label={`Delete ${trace.name}`}
											className="rounded p-0.5 text-muted-foreground opacity-0 hover:text-destructive focus-visible:opacity-100 group-hover:opacity-100"
										>
											<svg
												aria-hidden="true"
												width="12"
												height="12"
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
								))}
							</div>
						</div>
					)}
				</div>
			)}
		</div>
	);
};
