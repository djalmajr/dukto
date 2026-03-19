import { invoke } from "@tauri-apps/api/core";
import { createResource } from "solid-js";

export interface AppSettings {
	destination_dir: string;
}

async function fetchSettings(): Promise<AppSettings> {
	return invoke<AppSettings>("get_settings");
}

const [settings, { refetch: refetchSettings }] = createResource<AppSettings>(fetchSettings);

async function setDestinationDir(path: string) {
	await invoke("set_destination_dir", { path });
	refetchSettings();
}

export { settings, setDestinationDir };
