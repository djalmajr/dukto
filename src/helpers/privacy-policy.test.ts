import { expect, mock, test } from "bun:test";
import { readFileSync } from "node:fs";

test("privacy policy uses the native Android intent and the desktop shell opener", async () => {
	const invokes: Array<{ command: string; args?: unknown }> = [];
	const opened: string[] = [];

	mock.module("@tauri-apps/api/core", () => ({
		invoke: async (command: string, args?: unknown) => {
			invokes.push({ command, args });
		},
	}));
	mock.module("@tauri-apps/plugin-shell", () => ({
		open: async (url: string) => {
			opened.push(url);
		},
	}));

	try {
		const { openPrivacyPolicy, PRIVACY_POLICY_URL } = await import("./privacy-policy");
		expect(PRIVACY_POLICY_URL).toBe("https://dukto.app/docs/privacy/");
		const androidPlugin = readFileSync(
			new URL("../../src-tauri/android/DestinationPlugin.kt", import.meta.url),
			"utf8",
		);
		// Mutation captured: changing only the Android intent leaves the native privacy link stale.
		expect(androidPlugin).toContain(`Uri.parse("${PRIVACY_POLICY_URL}")`);

		await openPrivacyPolicy("android");
		expect(invokes).toEqual([{ command: "open_privacy_policy", args: undefined }]);
		expect(opened).toEqual([]);

		await openPrivacyPolicy("macos");
		expect(opened).toEqual([PRIVACY_POLICY_URL]);
	} finally {
		mock.restore();
	}
});
