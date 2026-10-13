import { describe, expect, it } from "bun:test";
import { assetSuffix, compareVersions, findUpdate } from "./update-check";

const release = {
	tag_name: "v0.3.0",
	html_url: "https://github.com/morrieinmaas/phonetic/releases/tag/v0.3.0",
	assets: [
		"Phonetic_0.3.0_aarch64.dmg",
		"Phonetic_0.3.0_x64.dmg",
		"Phonetic_0.3.0_amd64.deb",
		"Phonetic_0.3.0_arm64.deb",
		"Phonetic-0.3.0-1.x86_64.rpm",
		"Phonetic-0.3.0-1.aarch64.rpm",
		"Phonetic_0.3.0_amd64.AppImage",
		"Phonetic_0.3.0_aarch64.AppImage",
	].map((name) => ({ name, browser_download_url: `https://example.test/${name}` })),
};

describe("compareVersions", () => {
	it("compares numerically, not as text", () => {
		expect(compareVersions("v0.10.0", "0.9.0")).toBeGreaterThan(0);
		expect(compareVersions("0.2.0", "v0.2.0")).toBe(0);
		expect(compareVersions("0.2.0", "0.3.0")).toBeLessThan(0);
	});
});

describe("findUpdate", () => {
	it("offers nothing when the running version is current or newer", () => {
		const mac = { os: "macos", arch: "aarch64", linux_package: null } as const;
		expect(findUpdate(release, "0.3.0", mac)).toBeNull();
		expect(findUpdate(release, "0.4.0", mac)).toBeNull();
	});

	it("picks the exact installer for every supported machine", () => {
		const cases = [
			[{ os: "macos", arch: "aarch64", linux_package: null }, "Phonetic_0.3.0_aarch64.dmg"],
			[{ os: "macos", arch: "x86_64", linux_package: null }, "Phonetic_0.3.0_x64.dmg"],
			[{ os: "linux", arch: "x86_64", linux_package: "deb" }, "Phonetic_0.3.0_amd64.deb"],
			[{ os: "linux", arch: "aarch64", linux_package: "deb" }, "Phonetic_0.3.0_arm64.deb"],
			[{ os: "linux", arch: "x86_64", linux_package: "rpm" }, "Phonetic-0.3.0-1.x86_64.rpm"],
			[
				{ os: "linux", arch: "aarch64", linux_package: "appimage" },
				"Phonetic_0.3.0_aarch64.AppImage",
			],
		] as const;
		for (const [platform, file] of cases) {
			expect(findUpdate(release, "0.2.0", platform)?.fileName).toBe(file);
		}
	});

	it("falls back to the release page when no installer matches", () => {
		const update = findUpdate({ ...release, assets: [] }, "0.2.0", {
			os: "linux",
			arch: "x86_64",
			linux_package: "deb",
		});
		expect(update?.downloadUrl).toBe(release.html_url);
		expect(update?.fileName).toBeNull();
	});

	it("matches the suffixes the release workflow produces", () => {
		expect(assetSuffix({ os: "linux", arch: "x86_64", linux_package: null })).toBe(
			"_amd64.AppImage",
		);
	});
});
