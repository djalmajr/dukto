import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

interface AndroidDestinationEvent {
	path: string | null;
	error: string | null;
}

export async function pickDestinationDirectory(currentPlatform: string): Promise<string | null> {
	if (currentPlatform === "android") {
		return new Promise<string | null>((resolve, reject) => {
			let unlisten: (() => void) | undefined;
			void listen<AndroidDestinationEvent>("destination:selected", ({ payload }) => {
				unlisten?.();
				if (payload.error) reject(new Error(payload.error));
				else resolve(payload.path);
			})
				.then((stopListening) => {
					unlisten = stopListening;
					return invoke("open_destination_picker");
				})
				.catch((error) => {
					unlisten?.();
					reject(error);
				});
		});
	}

	const selected = await open({ directory: true, multiple: false });
	return Array.isArray(selected) ? (selected[0] ?? null) : selected;
}
