import { expect, mock, test } from "bun:test";

test("keeps concurrent transfers independent and incoming approvals ordered and recoverable", async () => {
	const handlers = new Map<string, (event: { payload: unknown }) => void>();
	const calls: Array<{ command: string; args: Record<string, unknown> }> = [];
	let finishFirstResponse!: () => void;
	const firstResponse = new Promise<void>((resolve) => {
		finishFirstResponse = resolve;
	});
	let finishZeroByteResponse!: () => void;
	const zeroByteResponse = new Promise<void>((resolve) => {
		finishZeroByteResponse = resolve;
	});
	let rejectNextResponse = false;
	mock.module("@tauri-apps/api/event", () => ({
		listen: async (name: string, callback: (event: { payload: unknown }) => void) => {
			handlers.set(name, callback);
			return () => handlers.delete(name);
		},
	}));
	mock.module("@tauri-apps/api/core", () => ({
		invoke: async (command: string, args: Record<string, unknown>) => {
			calls.push({ command, args });
			if (command === "respond_transfer" && args.transferId === "incoming-1") return firstResponse;
			if (command === "respond_transfer" && args.transferId === "zero-byte")
				return zeroByteResponse;
			if (command === "respond_transfer" && rejectNextResponse) {
				rejectNextResponse = false;
				throw new Error("temporary response failure");
			}
			return undefined;
		},
	}));

	const emit = (name: string, payload: unknown) => handlers.get(name)?.({ payload });
	const request = (
		transfer_id: string,
		sender_device_id = `peer-${transfer_id}`,
		total_size = 100,
	) => ({
		transfer_id,
		sender_device_id,
		sender: {
			device_id: sender_device_id,
			display_name: `user-${transfer_id}`,
			hostname: `${transfer_id}.local`,
			platform: "windows",
		},
		item_count: 1,
		total_size,
	});
	try {
		const store = await import("./transfers");
		emit("transfer:incoming", request("incoming-1"));
		emit("transfer:incoming", request("incoming-2"));
		emit("transfer:incoming", request("incoming-3"));
		expect(store.incomingRequest.current?.transfer_id).toBe("incoming-1");
		expect(store.incomingRequest.queue.map((item) => item.transfer_id)).toEqual([
			"incoming-2",
			"incoming-3",
		]);

		const pendingAccept = store.acceptIncoming();
		expect(calls.at(-1)).toEqual({
			command: "respond_transfer",
			args: { transferId: "incoming-1", accepted: true },
		});
		// A repeated click while the native response is pending cannot approve a later request.
		await store.acceptIncoming();
		emit("transfer:incoming", request("incoming-4"));
		expect(store.incomingRequest.current?.transfer_id).toBe("incoming-1");
		expect(store.incomingRequest.queue.map((item) => item.transfer_id)).toEqual([
			"incoming-2",
			"incoming-3",
			"incoming-4",
		]);
		finishFirstResponse();
		await pendingAccept;
		expect(store.incomingRequest.current?.transfer_id).toBe("incoming-2");
		expect(store.transfers["incoming-1"]).toMatchObject({
			peer_device_id: "peer-incoming-1",
			peer: {
				device_id: "peer-incoming-1",
				display_name: "user-incoming-1",
				hostname: "incoming-1.local",
				platform: "windows",
			},
			direction: "receive",
			status: "receiving",
		});
		const transientPeer = store.getUndiscoveredTransferPeers([])[0];
		expect(transientPeer).toEqual({
			device_id: "peer-incoming-1",
			display_name: "user-incoming-1",
			hostname: "incoming-1.local",
			platform: "windows",
		});
		expect(store.getUndiscoveredTransferPeers(["peer-incoming-1"])).toEqual([]);

		rejectNextResponse = true;
		await expect(store.rejectIncoming()).rejects.toThrow("temporary response failure");
		expect(store.incomingRequest.current?.transfer_id).toBe("incoming-2");
		expect(store.incomingRequest.queue.map((item) => item.transfer_id)).toEqual([
			"incoming-3",
			"incoming-4",
		]);
		await store.rejectIncoming();
		expect(store.incomingRequest.current?.transfer_id).toBe("incoming-3");
		await store.acceptIncoming();
		expect(store.incomingRequest.current?.transfer_id).toBe("incoming-4");
		expect(store.transfers["incoming-3"]?.status).toBe("receiving");

		emit("transfer:progress", {
			transfer_id: "incoming-1",
			bytes_sent: 30,
			bytes_total: 100,
			speed_bps: 5,
			percent: 30,
		});
		emit("transfer:progress", {
			transfer_id: "incoming-3",
			bytes_sent: 20,
			bytes_total: 100,
			speed_bps: 4,
			percent: 20,
		});
		emit("transfer:receive-error", { transfer_id: "incoming-3", error: "disk full" });
		expect(store.getPeerTransfers("peer-incoming-1")?.[0]).toMatchObject({
			status: "active",
			percent: 30,
		});
		expect(store.getUndiscoveredTransferPeers([])[0]).toBe(transientPeer);
		expect(store.getPeerTransfers("peer-incoming-3")?.[0]).toMatchObject({
			status: "error",
			errorMsg: "disk full",
		});
		emit("transfer:complete", {
			transfer_id: "incoming-3",
			items_received: 1,
			bytes_received: 100,
		});
		expect(store.getPeerTransfers("peer-incoming-3")?.[0]).toMatchObject({
			status: "error",
			errorMsg: "disk full",
		});

		// A timeout/rejection for a queued request removes only that request.
		emit("transfer:incoming", request("incoming-5"));
		emit("transfer:rejected", "incoming-5");
		expect(store.incomingRequest.current?.transfer_id).toBe("incoming-4");
		expect(store.incomingRequest.queue).toEqual([]);
		emit("transfer:rejected", "incoming-4");
		expect(store.incomingRequest.current).toBeNull();

		// Cancellation removes just its incoming request and retires the ID against duplicates.
		emit("transfer:incoming", request("waiting-receive-1"));
		emit("transfer:incoming", request("waiting-receive-2"));
		emit("transfer:cancelled", "waiting-receive-2");
		expect(store.incomingRequest.current?.transfer_id).toBe("waiting-receive-1");
		expect(store.incomingRequest.queue).toEqual([]);
		emit("transfer:incoming", request("waiting-receive-2"));
		expect(store.incomingRequest.queue).toEqual([]);
		emit("transfer:cancelled", "waiting-receive-1");
		expect(store.incomingRequest.current).toBeNull();
		emit("transfer:incoming", request("waiting-receive-1"));
		expect(store.incomingRequest.current).toBeNull();

		// A zero-byte receive can complete while the accept command is still resolving.
		emit("transfer:incoming", request("zero-byte", "peer-zero-byte", 0));
		const pendingZeroByteAccept = store.acceptIncoming();
		emit("transfer:complete", {
			transfer_id: "zero-byte",
			items_received: 0,
			bytes_received: 0,
		});
		expect(store.incomingRequest.current).toBeNull();
		expect(store.getPeerTransfers("peer-zero-byte")?.[0]).toMatchObject({
			status: "complete",
			percent: 100,
			bytesSent: 0,
			bytesTotal: 0,
		});
		finishZeroByteResponse();
		await pendingZeroByteAccept;
		expect(store.getPeerTransfers("peer-zero-byte")?.[0]).toMatchObject({
			status: "complete",
			percent: 100,
		});

		emit("transfer:send-started", { transfer_id: "send-large", peer_device_id: "host-a" });
		emit("transfer:progress", {
			transfer_id: "send-large",
			bytes_sent: 40,
			bytes_total: 100,
			speed_bps: 10,
			percent: 40,
		});
		emit("transfer:send-started", { transfer_id: "send-parallel", peer_device_id: "host-b" });
		emit("transfer:progress", {
			transfer_id: "send-parallel",
			bytes_sent: 25,
			bytes_total: 100,
			speed_bps: 8,
			percent: 25,
		});
		store.startSendTransfer("send-large", 100, "host-a");
		store.startSendTransfer("send-parallel", 100, "host-b");
		expect(store.getPeerTransfers("host-a")?.[0]).toMatchObject({
			status: "active",
			percent: 40,
			bytesSent: 40,
		});
		expect(store.getPeerTransfers("host-b")?.[0]).toMatchObject({
			status: "active",
			percent: 25,
			bytesSent: 25,
		});
		const sendPeer = {
			device_id: "host-a",
			display_name: "remote host",
			hostname: "remote.local",
			platform: "linux",
		};
		store.startSendTransfer("send-large", 100, "host-a", sendPeer);
		expect(store.getUndiscoveredTransferPeers([])).toContainEqual(sendPeer);
		expect(store.getUndiscoveredTransferPeers(["host-a"])).not.toContainEqual(sendPeer);

		emit("transfer:send-complete", { transfer_id: "send-parallel", bytes_sent: 100 });
		store.startSendTransfer("send-parallel", 100, "host-b");
		emit("transfer:send-error", { transfer_id: "send-large", error: "connection lost" });
		expect(store.getPeerTransfers("host-a")?.[0]).toMatchObject({ status: "error" });
		expect(store.getPeerTransfers("host-b")?.[0]).toMatchObject({
			status: "complete",
			percent: 100,
		});

		emit("transfer:send-started", { transfer_id: "cancel-one", peer_device_id: "host-a" });
		emit("transfer:send-started", { transfer_id: "cancel-two", peer_device_id: "host-a" });
		await store.abortTransfer("cancel-one");
		expect(calls.at(-1)).toEqual({
			command: "cancel_transfer",
			args: { transferId: "cancel-one" },
		});
		expect(store.getPeerTransfers("host-a")?.map((item) => item.id)).toContain("cancel-one");
		emit("transfer:cancelled", "cancel-one");
		expect(store.getPeerTransfers("host-a")?.map((item) => item.id)).not.toContain("cancel-one");
		expect(store.getPeerTransfers("host-a")?.map((item) => item.id)).toContain("cancel-two");
		emit("transfer:progress", {
			transfer_id: "cancel-one",
			bytes_sent: 99,
			bytes_total: 100,
			speed_bps: 5,
			percent: 99,
		});
		expect(store.transfers["cancel-one"]).toBeUndefined();
		store.startSendTransfer("cancel-one", 100, "host-a");
		emit("transfer:send-started", { transfer_id: "cancel-one", peer_device_id: "host-a" });
		expect(store.transfers["cancel-one"]).toBeUndefined();
		store.dismissPeerTransfer("incoming-1");
		expect(store.getUndiscoveredTransferPeers([])).not.toContainEqual({
			device_id: "peer-incoming-1",
			display_name: "user-incoming-1",
			hostname: "incoming-1.local",
			platform: "windows",
		});

		// Outgoing transfer enters waiting_approval until remote peer accepts
		emit("transfer:send-started", { transfer_id: "send-waiting", peer_device_id: "host-waiting" });
		store.startSendTransfer("send-waiting", 500, "host-waiting");
		expect(store.getPeerTransfers("host-waiting")?.[0]).toMatchObject({
			id: "send-waiting",
			status: "waiting_approval",
			bytesTotal: 500,
			bytesSent: 0,
			percent: 0,
		});

		// Upon remote acceptance, status transitions to active
		emit("transfer:send-accepted", { transfer_id: "send-waiting" });
		expect(store.getPeerTransfers("host-waiting")?.[0]).toMatchObject({
			id: "send-waiting",
			status: "active",
		});

		// Outgoing transfer rejected while waiting transitions to error
		emit("transfer:send-started", {
			transfer_id: "send-rejected",
			peer_device_id: "host-rejected",
		});
		store.startSendTransfer("send-rejected", 300, "host-rejected");
		expect(store.getPeerTransfers("host-rejected")?.[0]).toMatchObject({
			id: "send-rejected",
			status: "waiting_approval",
		});
		emit("transfer:send-error", {
			transfer_id: "send-rejected",
			error: "Transfer rejected by receiver",
		});
		expect(store.getPeerTransfers("host-rejected")?.[0]).toMatchObject({
			id: "send-rejected",
			status: "error",
			errorMsg: "Transfer rejected by receiver",
		});
	} finally {
		mock.restore();
	}
});
