import { open } from "@tauri-apps/plugin-shell";

export async function openDestinationDirectory(path: string): Promise<void> {
	return open(path);
}
