import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const androidConfig = JSON.parse(
	readFileSync(new URL("../../src-tauri/tauri.android.conf.json", import.meta.url), "utf8"),
);
const configureAndroidSource = readFileSync(
	new URL("../../scripts/configure-android.mjs", import.meta.url),
	"utf8",
);

describe("Android display name contract", () => {
	test("uses Dukto Share without changing the global desktop name", () => {
		expect(androidConfig.productName).toBe("Dukto Share");
	});

	test("reapplies the launcher and activity labels after Android regeneration", () => {
		expect(configureAndroidSource).toContain('const androidDisplayName = "Dukto Share";');
		expect(configureAndroidSource).toContain(
			'<string name="app_name">${androidDisplayName}</string>',
		);
		expect(configureAndroidSource).toContain(
			'<string name="main_activity_title">${androidDisplayName}</string>',
		);
	});
});
