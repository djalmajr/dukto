import { expect, mock, test } from "bun:test";

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

		await openPrivacyPolicy("android");
		expect(invokes).toEqual([{ command: "open_privacy_policy", args: undefined }]);
		expect(opened).toEqual([]);

		await openPrivacyPolicy("macos");
		expect(opened).toEqual([PRIVACY_POLICY_URL]);
	} finally {
		mock.restore();
	}
});
