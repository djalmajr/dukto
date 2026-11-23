import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const configureAndroid = readFileSync(new URL("./configure-android.mjs", import.meta.url), "utf8");

describe("Android debug build preparation", () => {
	test("copies the native destination plugin before building an APK", () => {
		expect(packageJson.scripts["android:build:debug"]).toMatch(
			/^node scripts\/configure-android\.mjs && bunx tauri android build --debug --apk$/,
		);
		expect(configureAndroid).toContain("await configureMainActivity();");
		expect(configureAndroid).toContain("sourceDestinationPluginPath");
		expect(configureAndroid).toContain('"DestinationPlugin.kt"');
	});
});
