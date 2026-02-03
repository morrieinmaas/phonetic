import { open } from "@tauri-apps/plugin-shell";

interface PermissionDialogProps {
	isOpen: boolean;
	onClose: () => void;
	onRetry: () => void;
}

export const PermissionDialog = ({ isOpen, onClose, onRetry }: PermissionDialogProps) => {
	if (!isOpen) return null;

	const openSystemPreferences = async () => {
		try {
			// Open macOS System Settings to Privacy > Microphone
			await open("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone");
		} catch {
			// Fallback: try opening System Preferences directly
			try {
				await open("x-apple.systempreferences:");
			} catch {
				// Silent fail - user can navigate manually
			}
		}
	};

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center">
			{/* Backdrop */}
			<div
				className="absolute inset-0 bg-black/50 backdrop-blur-sm"
				onClick={onClose}
				onKeyDown={(e) => e.key === "Escape" && onClose()}
				role="button"
				tabIndex={-1}
				aria-label="Close dialog"
			/>

			{/* Dialog */}
			<div className="relative glass rounded-lg border border-border shadow-xl w-full max-w-md mx-4">
				{/* Header */}
				<div className="flex items-center gap-3 p-4 border-b border-border">
					<div className="p-2 rounded-full bg-yellow-500/10">
						<svg
							xmlns="http://www.w3.org/2000/svg"
							width="24"
							height="24"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							strokeWidth="2"
							strokeLinecap="round"
							strokeLinejoin="round"
							className="text-yellow-500"
						>
							<title>Microphone</title>
							<path d="M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" />
							<path d="M19 10v2a7 7 0 0 1-14 0v-2" />
							<line x1="12" x2="12" y1="19" y2="22" />
						</svg>
					</div>
					<div>
						<h2 className="text-lg font-semibold">Microphone Access Required</h2>
						<p className="text-sm text-muted-foreground">
							Phonetic needs permission to access your microphone
						</p>
					</div>
				</div>

				{/* Content */}
				<div className="p-4 space-y-4">
					<p className="text-sm text-muted-foreground">
						To analyze audio signals and measure frequency response, Phonetic requires microphone
						access.
					</p>

					<div className="space-y-2 text-sm">
						<p className="font-medium">To enable microphone access:</p>
						<ol className="list-decimal list-inside space-y-1 text-muted-foreground">
							<li>
								Open <strong>System Settings</strong>
							</li>
							<li>
								Go to <strong>Privacy & Security</strong>
							</li>
							<li>
								Select <strong>Microphone</strong>
							</li>
							<li>
								Enable <strong>Phonetic</strong> in the list
							</li>
						</ol>
					</div>

					<button
						type="button"
						onClick={openSystemPreferences}
						className="w-full flex items-center justify-center gap-2 px-4 py-2 text-sm rounded-md border border-border hover:bg-muted transition-colors"
					>
						<svg
							xmlns="http://www.w3.org/2000/svg"
							width="16"
							height="16"
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
						Open System Settings
					</button>
				</div>

				{/* Footer */}
				<div className="flex gap-2 p-4 border-t border-border">
					<button
						type="button"
						onClick={onClose}
						className="flex-1 px-4 py-2 text-sm font-medium rounded-md border border-border hover:bg-muted transition-colors"
					>
						Cancel
					</button>
					<button
						type="button"
						onClick={onRetry}
						className="flex-1 px-4 py-2 text-sm font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
					>
						Try Again
					</button>
				</div>
			</div>
		</div>
	);
};
