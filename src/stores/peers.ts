import { listen } from "@tauri-apps/api/event";
import { createStore } from "solid-js/store";

export interface PeerInfo {
	device_id: string;
	display_name: string;
	hostname: string;
	platform: string;
	addresses: string[];
	port: number;
	protocol_version: string;
}

const [peers, setPeers] = createStore<Record<string, PeerInfo>>({});

// Listen for discovery events from Rust
listen<PeerInfo>("peer:found", (event) => {
	setPeers(event.payload.device_id, event.payload);
});

listen<string>("peer:removed", (event) => {
	// Remove peer by finding which one has a matching fullname
	// For now, remove by iterating (fullname contains device_id)
	const fullname = event.payload;
	for (const [id] of Object.entries(peers)) {
		if (fullname.includes(id)) {
			setPeers((prev) => {
				const next = { ...prev };
				delete next[id];
				return next;
			});
			break;
		}
	}
});

export { peers };
