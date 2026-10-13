import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { create } from "zustand";
import {
	type AvailableUpdate,
	type PlatformInfo,
	RELEASES_API,
	type Release,
	findUpdate,
} from "../lib/update-check";

const SKIPPED_KEY = "phonetic-skipped-version";

type CheckStatus = "idle" | "checking" | "current" | "available" | "failed";

interface UpdateStore {
	currentVersion: string | null;
	status: CheckStatus;
	update: AvailableUpdate | null;
	/** The notice is hidden for this session ("Later") or for this version ("Skip"). */
	noticeHidden: boolean;
	/**
	 * Ask GitHub for the latest release. A silent check (on launch) respects a
	 * skipped version and stays quiet on failure; a manual one reports both.
	 */
	check: (manual: boolean) => Promise<void>;
	later: () => void;
	skip: () => void;
}

const readSkipped = (): string | null => {
	try {
		return localStorage.getItem(SKIPPED_KEY);
	} catch {
		return null;
	}
};

export const useUpdateStore = create<UpdateStore>((set, get) => ({
	currentVersion: null,
	status: "idle",
	update: null,
	noticeHidden: false,

	check: async (manual) => {
		if (get().status === "checking") return;
		set({ status: "checking" });
		try {
			const [currentVersion, platform, response] = await Promise.all([
				getVersion(),
				invoke<PlatformInfo>("get_platform"),
				fetch(RELEASES_API, { headers: { Accept: "application/vnd.github+json" } }),
			]);
			if (!response.ok) throw new Error(`GitHub answered ${response.status}`);
			const update = findUpdate((await response.json()) as Release, currentVersion, platform);
			const skipped = !manual && update !== null && readSkipped() === update.version;
			set({
				currentVersion,
				update,
				status: update ? "available" : "current",
				noticeHidden: skipped,
			});
		} catch {
			// Offline at a gig is normal; only a check the operator asked for reports it.
			set({ status: manual ? "failed" : "idle" });
		}
	},

	later: () => set({ noticeHidden: true }),

	skip: () => {
		const version = get().update?.version;
		try {
			if (version) localStorage.setItem(SKIPPED_KEY, version);
		} catch {
			// Storage blocked: the notice still hides for this session.
		}
		set({ noticeHidden: true });
	},
}));
