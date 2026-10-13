import type { AudioDevice } from "../types/audio";

interface ChannelSelectorProps {
	/** The currently selected input device, so the count select can be bounded by its hardware maximum. */
	inputDevice: AudioDevice | null;
	channelCount: number;
	measurementChannel: number;
	onChannelCountChange: (count: number) => void;
	onMeasurementChannelChange: (index: number) => void;
	disabled?: boolean;
}

/**
 * Lets the operator open several input channels at once (up to what the
 * device supports) and pick which one drives level metering and rig
 * diagnostics. Spatial averaging across positions happens after capture, in
 * TraceControls.
 */
export const ChannelSelector = ({
	inputDevice,
	channelCount,
	measurementChannel,
	onChannelCountChange,
	onMeasurementChannelChange,
	disabled = false,
}: ChannelSelectorProps) => {
	const maxChannels = Math.max(1, inputDevice?.channels ?? 1);
	const channelCountOptions = Array.from({ length: maxChannels }, (_, i) => i + 1);
	const measurementChannelOptions = Array.from({ length: channelCount }, (_, i) => i);

	return (
		<div data-testid="channel-selector" className="flex flex-col gap-1.5">
			<label
				htmlFor="channel-count-select"
				className="text-xs font-medium uppercase tracking-wider text-muted-foreground"
			>
				Input Channels
			</label>
			<select
				id="channel-count-select"
				data-testid="channel-count"
				value={channelCount}
				onChange={(e) => onChannelCountChange(Number(e.target.value))}
				disabled={disabled}
				className="glass h-9 rounded-md border border-border bg-background/50 px-3 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
			>
				{channelCountOptions.map((count) => (
					<option key={count} value={count}>
						{count} {count === 1 ? "channel" : "channels"}
					</option>
				))}
			</select>

			{channelCount > 1 && (
				<>
					<label
						htmlFor="measurement-channel-select"
						className="text-xs font-medium uppercase tracking-wider text-muted-foreground"
					>
						Measurement Channel
					</label>
					<select
						id="measurement-channel-select"
						data-testid="measurement-channel"
						value={measurementChannel}
						onChange={(e) => onMeasurementChannelChange(Number(e.target.value))}
						disabled={disabled}
						className="glass h-9 rounded-md border border-border bg-background/50 px-3 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
					>
						{measurementChannelOptions.map((index) => (
							<option key={index} value={index}>
								Channel {index + 1}
							</option>
						))}
					</select>
				</>
			)}
		</div>
	);
};
