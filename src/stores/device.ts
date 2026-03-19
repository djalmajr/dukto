import { createResource } from "solid-js";
import { type DeviceIdentity, getDeviceInfo } from "~/lib/tauri";

const [device] = createResource<DeviceIdentity>(getDeviceInfo);

export { device };
