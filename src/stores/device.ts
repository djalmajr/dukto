import { createResource } from "solid-js";
import { type DeviceIdentity, getDeviceInfo } from "~/helpers/tauri";

const [device] = createResource<DeviceIdentity>(getDeviceInfo);

export { device };
