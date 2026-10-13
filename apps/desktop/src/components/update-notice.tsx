import { open } from "@tauri-apps/plugin-shell";
import { useEffect } from "react";
import { useUpdateStore } from "../stores/update-store";

/**
 * "A new version is available" toast. Checks once on launch; Download opens the exact
 * installer for this machine in the browser, Later hides it until next launch, Skip hides
 * it until a newer version than the skipped one appears.
 */
export const UpdateNotice = () => {
	const { status, update, noticeHidden, currentVersion, check, later, skip } = useUpdateStore();

	useEffect(() => {
		void check(false);
	}, [check]);

	if (status !== "available" || !update || noticeHidden) return null;

	return (
		<output
			data-testid="update-notice"
			className="pointer-events-auto flex flex-col gap-2.5 rounded-lg border border-primary/60 bg-card p-3.5 text-sm text-foreground shadow-[0_8px_24px_-8px_rgb(0_0_0/0.45)]"
		>
			<div className="flex items-start gap-3">
				<svg
					aria-hidden="true"
					width="18"
					height="18"
					viewBox="0 0 24 24"
					fill="none"
					stroke="currentColor"
					strokeWidth="2"
					strokeLinecap="round"
					strokeLinejoin="round"
					className="mt-0.5 flex-shrink-0 text-primary"
				>
					<path d="M12 3v12M7 10l5 5 5-5M5 21h14" />
				</svg>
				<div className="flex flex-col gap-0.5">
					<p className="font-medium">Phonetic {update.version} is available</p>
					<p className="text-xs text-muted-foreground">
						You have {currentVersion}.{" "}
						{update.fileName
							? "Download the installer and install it like the first time."
							: "Pick the installer for your system on the release page."}
					</p>
				</div>
			</div>
			<div className="flex flex-wrap items-center gap-2 pl-[30px]">
				<button
					type="button"
					data-testid="update-download"
					onClick={() => void open(update.downloadUrl)}
					className="rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90"
				>
					Download
				</button>
				<button
					type="button"
					onClick={() => void open(update.notesUrl)}
					className="rounded-md border border-border px-2.5 py-1 text-xs hover:bg-muted"
				>
					What's new
				</button>
				<button
					type="button"
					onClick={later}
					className="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
				>
					Later
				</button>
				<button
					type="button"
					onClick={skip}
					className="rounded-md px-2 py-1 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
				>
					Skip this version
				</button>
			</div>
		</output>
	);
};
