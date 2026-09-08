import { useCallback, useState } from "react";
import { applyMeasuredDelay } from "../services/delay-apply";
import { useTraceStore } from "../stores/trace-store";
import type { TraceSlot, TransferFunctionData } from "../types/audio";

interface TraceControlsProps {
	currentData: TransferFunctionData | null;
	isRunning: boolean;
}

const SLOTS: TraceSlot[] = ["A", "B", "C", "D"];

export const TraceControls = ({ currentData, isRunning }: TraceControlsProps) => {
	const { traces, slots, captureToSlot, toggleTraceVisibility, deleteTrace, clearAllTraces } =
		useTraceStore();
	const [showList, setShowList] = useState(false);
	const [delayInput, setDelayInput] = useState("");
	const [delayStatus, setDelayStatus] = useState<"idle" | "applied" | "rejected">("idle");
	const [appliedDelay, setAppliedDelay] = useState<number | null>(null);

	const handleApplyDelay = useCallback(async () => {
		const delaySamples = Number(delayInput);
		const applied = await applyMeasuredDelay({
			delay_samples: delaySamples,
			is_valid: Number.isFinite(delaySamples),
		});
		if (applied) {
			setAppliedDelay(delaySamples);
			setDelayStatus("applied");
		} else {
			setDelayStatus("rejected");
		}
	}, [delayInput]);

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
				<span className="text-xs text-muted-foreground mr-1">Store:</span>
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

			{/* Apply a measured delay to compensate the transfer function reference */}
			<div className="flex items-center gap-1 ml-2 border-l border-border pl-2">
				<span className="text-xs text-muted-foreground mr-1">Delay:</span>
				<input
					type="number"
					min={0}
					value={delayInput}
					onChange={(e) => setDelayInput(e.target.value)}
					placeholder="samples"
					className="w-16 px-1 py-0.5 text-xs rounded border border-border bg-background"
				/>
				<button
					type="button"
					data-testid="apply-delay"
					onClick={handleApplyDelay}
					disabled={delayInput === ""}
					title="Apply the measured delay to compensate the transfer function reference"
					className="px-2 py-1 text-xs rounded border border-border hover:bg-accent disabled:opacity-40 disabled:cursor-not-allowed"
				>
					Apply
				</button>
				{delayStatus === "applied" && (
					<span className="text-[10px] text-muted-foreground">Applied {appliedDelay} smp</span>
				)}
				{delayStatus === "rejected" && (
					<span className="text-[10px] text-destructive">Invalid delay</span>
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
						className="px-2 py-1 text-xs rounded border border-border hover:bg-accent"
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
										className="flex items-center gap-2 p-2 hover:bg-accent/50 group"
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
											className="opacity-0 group-hover:opacity-100 text-xs text-destructive hover:underline"
										>
											×
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
