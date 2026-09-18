import { invoke } from "@tauri-apps/api/core";
import { createResource } from "solid-js";
import { type DeviceIdentity, getDeviceInfo } from "~/helpers/tauri";

const [device, { mutate }] = createResource<DeviceIdentity>(getDeviceInfo);

async function setDeviceDisplayName(displayName: string) {
	const updated = await invoke<DeviceIdentity>("set_display_name", { displayName });
	mutate(updated);
	return updated;
}

export { device, setDeviceDisplayName };
