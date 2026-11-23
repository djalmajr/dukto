import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../", import.meta.url);

function read(path: string): string {
	return readFileSync(fileURLToPath(new URL(path, root)), "utf8");
}

describe("installed app names", () => {
	test("uses Dukto Share on Android, iOS, and macOS", () => {
		// Mutation captured: reverting either Apple product name makes its installed label differ from Android.
		const android = JSON.parse(read("src-tauri/tauri.android.conf.json"));
		const ios = JSON.parse(read("src-tauri/tauri.ios.conf.json"));
		const macos = JSON.parse(read("src-tauri/tauri.macos.conf.json"));
		const iosInfo = read("src-tauri/Info.ios.plist");
		const releaseWorkflow = read(".github/workflows/build-installers.yml");

		expect(android.productName).toBe("Dukto Share");
		expect(ios.productName).toBe("Dukto Share");
		expect(macos.productName).toBe("Dukto Share");
		expect(iosInfo).toMatch(/<key>CFBundleDisplayName<\/key>\s*<string>Dukto Share<\/string>/);
		expect(iosInfo).toMatch(/<key>CFBundleName<\/key>\s*<string>Dukto Share<\/string>/);
		expect(releaseWorkflow).toContain("Dukto Share.app/Contents/MacOS");
		expect(releaseWorkflow).toContain("Dukto Share.app.tar.gz");
	});
});
