import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-shell";

export const PRIVACY_POLICY_URL = "https://dukto.app/docs/privacidade/";

export async function openPrivacyPolicy(currentPlatform: string): Promise<void> {
	if (currentPlatform === "android") {
		return invoke("open_privacy_policy");
	}
	return open(PRIVACY_POLICY_URL);
}
