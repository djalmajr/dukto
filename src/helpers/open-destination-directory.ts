import { invoke } from "@tauri-apps/api/core";

export async function openDestinationDirectory(): Promise<void> {
	return invoke("open_destination_directory");
}
