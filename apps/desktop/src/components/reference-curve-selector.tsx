import { useEffect, useState } from "react";
import {
	type ReferenceCurve,
	type ReferenceCurveInfo,
	type ReferenceCurveType,
	audioApi,
} from "../services/audio-api";

interface ReferenceCurveSelectorProps {
	selectedCurve: ReferenceCurveType | null;
	onCurveChange: (curve: ReferenceCurveType | null) => void;
	onCurveDataChange?: (data: ReferenceCurve | null) => void;
	numPoints?: number;
}

export const ReferenceCurveSelector = ({
	selectedCurve,
	onCurveChange,
	onCurveDataChange,
	numPoints = 256,
}: ReferenceCurveSelectorProps) => {
	const [availableCurves, setAvailableCurves] = useState<ReferenceCurveInfo[]>([]);
	const [isLoading, setIsLoading] = useState(false);

	// Fetch available curves on mount (wait for Tauri to be ready)
	useEffect(() => {
		// Check if Tauri runtime is available
		if (!window.__TAURI_INTERNALS__) {
			// Retry after a short delay if Tauri isn't ready yet
			const timer = setTimeout(() => {
				if (window.__TAURI_INTERNALS__) {
					audioApi.getAvailableReferenceCurves().then(setAvailableCurves).catch(console.error);
				}
			}, 100);
			return () => clearTimeout(timer);
		}
		audioApi.getAvailableReferenceCurves().then(setAvailableCurves).catch(console.error);
	}, []);

	// Fetch curve data when selection changes
	useEffect(() => {
		if (!selectedCurve) {
			onCurveDataChange?.(null);
			return;
		}

		setIsLoading(true);
		audioApi
			.getReferenceCurve(selectedCurve, numPoints)
			.then((data) => {
				onCurveDataChange?.(data);
			})
			.catch(console.error)
			.finally(() => setIsLoading(false));
	}, [selectedCurve, numPoints, onCurveDataChange]);

	return (
		<div className="flex items-center gap-2">
			<span className="text-xs text-muted-foreground">Ref:</span>
			<select
				value={selectedCurve ?? ""}
				onChange={(e) =>
					onCurveChange(e.target.value ? (e.target.value as ReferenceCurveType) : null)
				}
				disabled={isLoading}
				className="px-2 py-1 text-xs rounded border border-border bg-background text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
			>
				<option value="">None</option>
				{availableCurves.map((curve) => (
					<option key={curve.curve_type} value={curve.curve_type}>
						{curve.name}
					</option>
				))}
			</select>
			{isLoading && <span className="text-xs text-muted-foreground animate-pulse">...</span>}
		</div>
	);
};
