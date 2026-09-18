import { expect, mock, test } from "bun:test";
import type { PeerInfo } from "./peers";

test("hydrates existing peers and preserves updates arriving during the snapshot", async () => {
	const handlers = new Map<string, (event: { payload: unknown }) => void>();
	let resolveSnapshot!: (peers: PeerInfo[]) => void;
	const snapshot = new Promise<PeerInfo[]>((resolve) => {
		resolveSnapshot = resolve;
	});
	let snapshotRequested!: () => void;
	const requested = new Promise<void>((resolve) => {
		snapshotRequested = resolve;
	});
	mock.module("@tauri-apps/api/event", () => ({
		listen: async (name: string, callback: (event: { payload: unknown }) => void) => {
			handlers.set(name, callback);
			return () => handlers.delete(name);
		},
	}));
	mock.module("@tauri-apps/api/core", () => ({
		invoke: (command: string) => {
			expect(command).toBe("get_peers");
			expect(handlers.size).toBe(2);
			snapshotRequested();
			return snapshot;
		},
	}));
	const peer = (id: string, name = id): PeerInfo => ({
		device_id: id,
		display_name: name,
		hostname: `${id}.local`,
		platform: "windows",
		addresses: ["192.168.0.15"],
		port: 4242,
		protocol_version: "0.2",
	});
	try {
		const { peers, peersReady } = await import("./peers");
		await requested;
		handlers.get("peer:found")?.({ payload: peer("updated", "New name") });
		handlers.get("peer:removed")?.({ payload: "removed._dukto._tcp.local." });
		resolveSnapshot([peer("existing"), peer("updated", "Old name"), peer("removed")]);
		await peersReady;
		expect(peers.existing.display_name).toBe("existing");
		expect(peers.updated.display_name).toBe("New name");
		expect(peers.removed).toBeUndefined();
		handlers.get("peer:found")?.({ payload: peer("late") });
		expect(peers.late.addresses).toEqual(["192.168.0.15"]);
		handlers.get("peer:found")?.({
			payload: { ...peer("late", "Updated name"), addresses: ["fe80::1234"] },
		});
		expect(peers.late.display_name).toBe("Updated name");
		expect(peers.late.addresses).toEqual(["fe80::1234", "192.168.0.15"]);
	} finally {
		mock.restore();
	}
});
