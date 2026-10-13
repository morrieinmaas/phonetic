import type { AudioDevice } from "../types/audio";

interface DeviceSelectorProps {
	label: string;
	devices: AudioDevice[];
	selectedDeviceId: string | null;
	defaultDeviceId: string | null;
	onSelect: (deviceId: string | null) => void;
	disabled?: boolean;
	testId?: string;
}

export const DeviceSelector = ({
	label,
	devices,
	selectedDeviceId,
	defaultDeviceId,
	onSelect,
	disabled = false,
	testId,
}: DeviceSelectorProps) => {
	const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
		const value = e.target.value;
		onSelect(value === "" ? null : value);
	};

	// Generate a stable ID from the label
	const inputId = `device-selector-${label.toLowerCase().replace(/\s+/g, "-")}`;

	return (
		<div className="flex flex-col gap-1.5">
			<label
				htmlFor={inputId}
				className="text-xs font-medium uppercase tracking-wider text-muted-foreground"
			>
				{label}
			</label>
			<select
				id={inputId}
				data-testid={testId}
				value={selectedDeviceId ?? ""}
				onChange={handleChange}
				disabled={disabled}
				className="glass h-9 rounded-md border border-border bg-background/50 px-3 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-ring disabled:cursor-not-allowed disabled:opacity-50"
			>
				<option value="">System Default</option>
				{devices.map((device) => (
					<option key={device.id} value={device.id}>
						{device.name}
						{device.id === defaultDeviceId ? " (Default)" : ""}
						{device.sample_rates.length > 0
							? ` - ${Math.max(...device.sample_rates) / 1000}kHz`
							: ""}
					</option>
				))}
			</select>
		</div>
	);
};
