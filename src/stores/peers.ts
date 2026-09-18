import { listen } from "@tauri-apps/api/event";
import { createStore, produce } from "solid-js/store";
import { getPeers } from "~/helpers/tauri";

export interface PeerIdentity {
	device_id: string;
	display_name: string;
	hostname: string;
	platform: string;
}

export interface PeerInfo extends PeerIdentity {
	addresses: string[];
	port: number;
	protocol_version: string;
}

const [peers, setPeers] = createStore<Record<string, PeerInfo>>({});

function peerFound(peer: PeerInfo) {
	const id = peer.device_id;

	const previous = peers[id];
	setPeers(id, {
		...peer,
		addresses: [...new Set([...peer.addresses, ...(previous?.addresses ?? [])])],
	});
}

function peerRemoved(fullname: string) {
	for (const id of Object.keys(peers)) {
		if (fullname.includes(id)) {
			setPeers(
				produce((state) => {
					delete state[id];
				}),
			);
			break;
		}
	}
}

async function initializePeers() {
	// Subscribe first; replay events received during the snapshot request so an
	// older snapshot cannot overwrite an update or resurrect a removed peer.
	let pending: Array<() => void> | undefined = [];
	const apply = (update: () => void) => {
		if (pending) pending.push(update);
		else update();
	};
	await Promise.all([
		listen<PeerInfo>("peer:found", ({ payload }) => apply(() => peerFound(payload))),
		listen<string>("peer:removed", ({ payload }) => apply(() => peerRemoved(payload))),
	]);
	try {
		for (const peer of await getPeers()) peerFound(peer);
	} finally {
		const updates = pending;
		pending = undefined;
		for (const update of updates) update();
	}
}

export const peersReady = initializePeers().catch((error) => {
	console.error("Failed to initialize peer discovery", error);
});

export { peers };
