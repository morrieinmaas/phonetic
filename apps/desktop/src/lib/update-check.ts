/** Where releases are published. The update check reads only this public endpoint. */
export const RELEASES_API = "https://api.github.com/repos/morrieinmaas/phonetic/releases/latest";

export interface PlatformInfo {
	os: string;
	arch: string;
	linux_package: "appimage" | "deb" | "rpm" | null;
}

interface ReleaseAsset {
	name: string;
	browser_download_url: string;
}

export interface Release {
	tag_name: string;
	html_url: string;
	assets: ReleaseAsset[];
}

export interface AvailableUpdate {
	version: string;
	/** The installer for this machine, or the release page when none matches. */
	downloadUrl: string;
	fileName: string | null;
	notesUrl: string;
}

/** Numeric x.y.z comparison; a leading "v" is ignored. Positive when a is newer. */
export const compareVersions = (a: string, b: string): number => {
	const parts = (v: string) =>
		v
			.replace(/^v/, "")
			.split(".")
			.map((n) => Number.parseInt(n, 10) || 0);
	const [pa, pb] = [parts(a), parts(b)];
	for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
		const diff = (pa[i] ?? 0) - (pb[i] ?? 0);
		if (diff !== 0) return diff;
	}
	return 0;
};

/** File-name suffix of the installer that fits, matching what the release workflow uploads. */
export const assetSuffix = (platform: PlatformInfo): string => {
	const arm = platform.arch === "aarch64";
	if (platform.os === "macos") return arm ? "_aarch64.dmg" : "_x64.dmg";
	if (platform.linux_package === "deb") return arm ? "_arm64.deb" : "_amd64.deb";
	if (platform.linux_package === "rpm") return arm ? ".aarch64.rpm" : ".x86_64.rpm";
	return arm ? "_aarch64.AppImage" : "_amd64.AppImage";
};

/** The update to offer, or null when the running version is current. */
export const findUpdate = (
	release: Release,
	currentVersion: string,
	platform: PlatformInfo,
): AvailableUpdate | null => {
	if (compareVersions(release.tag_name, currentVersion) <= 0) return null;
	const suffix = assetSuffix(platform);
	const asset = release.assets.find((candidate) => candidate.name.endsWith(suffix));
	return {
		version: release.tag_name.replace(/^v/, ""),
		downloadUrl: asset?.browser_download_url ?? release.html_url,
		fileName: asset?.name ?? null,
		notesUrl: release.html_url,
	};
};
