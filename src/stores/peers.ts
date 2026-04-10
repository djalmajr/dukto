import { listen } from "@tauri-apps/api/event";
import { createStore, produce } from "solid-js/store";

export interface PeerInfo {
	device_id: string;
	display_name: string;
	hostname: string;
	platform: string;
	addresses: string[];
	port: number;
	protocol_version: string;
}

const REMOVAL_GRACE_MS = 60_000;

const [peers, setPeers] = createStore<Record<string, PeerInfo>>({});
const removalTimers = new Map<string, number>();

listen<PeerInfo>("peer:found", (event) => {
	const id = event.payload.device_id;

	// Cancel pending removal if peer re-appeared
	const timer = removalTimers.get(id);
	if (timer) {
		clearTimeout(timer);
		removalTimers.delete(id);
	}

	setPeers(id, event.payload);
});

listen<string>("peer:removed", (event) => {
	const fullname = event.payload;
	for (const id of Object.keys(peers)) {
		if (fullname.includes(id)) {
			// Already scheduled? skip
			if (removalTimers.has(id)) break;

			const timer = window.setTimeout(() => {
				removalTimers.delete(id);
				setPeers(produce((state) => { delete state[id]; }));
			}, REMOVAL_GRACE_MS);
			removalTimers.set(id, timer);
			break;
		}
	}
});

export { peers };
