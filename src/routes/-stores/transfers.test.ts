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
			bytes_sent: 0,
			bytes_total: 100,
			speed_bps: 0,
			percent: 0,
			item_name: "large.mov",
		});
		// Mutation captured: dropping item_name from receive progress hides the file being received.
		expect(store.getPeerTransfers("peer-incoming-1")?.[0]?.currentItemName).toBe("large.mov");
		emit("transfer:progress", {
			transfer_id: "incoming-1",
			bytes_sent: 30,
			bytes_total: 100,
			speed_bps: 5,
			percent: 30,
		});
		expect(store.getPeerTransfers("peer-incoming-1")?.[0]?.currentItemName).toBe("large.mov");
		emit("transfer:progress", {
			transfer_id: "incoming-1",
			bytes_sent: 30,
			bytes_total: 100,
			speed_bps: 5,
			percent: 30,
			item_name: "next.txt",
		});
		expect(store.getPeerTransfers("peer-incoming-1")?.[0]?.currentItemName).toBe("next.txt");
		emit("transfer:progress", {
			transfer_id: "incoming-3",
			bytes_sent: 20,
			bytes_total: 100,
			speed_bps: 4,
			percent: 20,
			items: [
				{
					item_id: "received-item",
					name: "same-name.txt",
					kind: "File",
					size_bytes: 20,
					item_count: 1,
				},
				{
					item_id: "pending-item",
					name: "same-name.txt",
					kind: "File",
					size_bytes: 80,
					item_count: 1,
				},
			],
			item_progress: {
				item_id: "received-item",
				bytes_sent: 20,
				bytes_total: 20,
				completed: true,
			},
		});
		emit("transfer:progress", {
			transfer_id: "incoming-3",
			bytes_sent: 20,
			bytes_total: 100,
			speed_bps: 4,
			percent: 20,
			item_progress: {
				item_id: "pending-item",
				bytes_sent: 20,
				bytes_total: 80,
				completed: false,
			},
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
			items: [
				{ item_id: "received-item", completed: true },
				{ item_id: "pending-item", bytes_sent: 20, completed: false },
			],
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
		store.startSendTransfer("send-large", 100, "host-a", undefined, ["large.mov"]);
		store.startSendTransfer("send-parallel", 100, "host-b", undefined, ["notes.txt", "photos"]);
		expect(store.getPeerTransfers("host-a")?.[0]).toMatchObject({
			status: "active",
			percent: 40,
			bytesSent: 40,
			itemNames: ["large.mov"],
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
			itemNames: ["notes.txt", "photos"],
		});
		store.dismissPeerTransfer("send-parallel");
		expect(store.getPeerTransfers("host-b")).toBeUndefined();
		emit("transfer:send-started", { transfer_id: "send-fast", peer_device_id: "host-b" });
		emit("transfer:send-complete", { transfer_id: "send-fast", bytes_sent: 1 });
		store.startSendTransfer("send-fast", 1, "host-b", undefined, ["quick.txt"]);
		expect(store.getPeerTransfers("host-b")?.[0]).toMatchObject({
			status: "complete",
			itemNames: ["quick.txt"],
		});
		store.dismissPeerTransfer("send-fast");

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
		// Item descriptors keep same-named selections independent and expose folder and empty-item progress.
		emit("transfer:send-started", { transfer_id: "send-items", peer_device_id: "host-items" });
		store.startSendTransfer("send-items", 12200000, "host-items");
		const itemDescriptors = [
			{
				item_id: "file-a",
				name: "README.md",
				kind: "File" as const,
				size_bytes: 200,
				item_count: 1,
			},
			{
				item_id: "file-b",
				name: "README.md",
				kind: "File" as const,
				size_bytes: 200,
				item_count: 1,
			},
			{
				item_id: "folder-a",
				name: "css",
				kind: "Directory" as const,
				size_bytes: 1000,
				item_count: 3,
			},
			{
				item_id: "empty-a",
				name: "empty.txt",
				kind: "File" as const,
				size_bytes: 0,
				item_count: 1,
			},
		];
		emit("transfer:progress", {
			transfer_id: "send-items",
			bytes_sent: 200,
			bytes_total: 12200000,
			speed_bps: 100,
			percent: 1,
			items: itemDescriptors,
			item_progress: {
				item_id: "file-a",
				bytes_sent: 200,
				bytes_total: 200,
				completed: true,
			},
		});
		emit("transfer:progress", {
			transfer_id: "send-items",
			bytes_sent: 700,
			bytes_total: 12200000,
			speed_bps: 100,
			percent: 2,
			item_progress: {
				item_id: "folder-a",
				bytes_sent: 500,
				bytes_total: 1000,
				completed: false,
			},
		});
		emit("transfer:progress", {
			transfer_id: "send-items",
			bytes_sent: 700,
			bytes_total: 12200000,
			speed_bps: 100,
			percent: 2,
			item_progress: {
				item_id: "empty-a",
				bytes_sent: 0,
				bytes_total: 0,
				completed: true,
			},
		});
		const itemTransfer = store.getPeerTransfers("host-items")?.[0];
		expect(itemTransfer?.items?.map((item) => item.item_id)).toEqual([
			"file-a",
			"file-b",
			"folder-a",
			"empty-a",
		]);
		expect(itemTransfer?.items).toEqual([
			{ ...itemDescriptors[0], bytes_sent: 200, bytes_total: 200, completed: true },
			{ ...itemDescriptors[1], bytes_sent: 0, bytes_total: 200, completed: false },
			{ ...itemDescriptors[2], bytes_sent: 500, bytes_total: 1000, completed: false },
			{ ...itemDescriptors[3], bytes_sent: 0, bytes_total: 0, completed: true },
		]);

		// A completed transfer marks all known items, and late descriptor merges retain the terminal state.
		emit("transfer:send-complete", { transfer_id: "send-items", bytes_sent: 12200000 });
		emit("transfer:progress", {
			transfer_id: "send-items",
			bytes_sent: 12200000,
			bytes_total: 12200000,
			speed_bps: 0,
			percent: 100,
			items: [
				{
					item_id: "late-item",
					name: "later.txt",
					kind: "File",
					size_bytes: 42,
					item_count: 1,
				},
			],
			item_progress: {
				item_id: "late-item",
				bytes_sent: 42,
				bytes_total: 42,
				completed: true,
			},
		});
		expect(store.getPeerTransfers("host-items")?.[0]?.items?.every((item) => item.completed)).toBe(
			true,
		);

		// The same filename in a concurrent transfer remains isolated by item_id and transfer_id.
		emit("transfer:send-started", {
			transfer_id: "send-items-other",
			peer_device_id: "host-items-other",
		});
		store.startSendTransfer("send-items-other", 200, "host-items-other");
		emit("transfer:progress", {
			transfer_id: "send-items-other",
			bytes_sent: 0,
			bytes_total: 200,
			speed_bps: 0,
			percent: 0,
			items: [{ ...itemDescriptors[1], item_id: "other-file" }],
			item_progress: {
				item_id: "other-file",
				bytes_sent: 0,
				bytes_total: 200,
				completed: false,
			},
		});
		expect(store.getPeerTransfers("host-items-other")?.[0]?.items?.[0]).toMatchObject({
			item_id: "other-file",
			name: "README.md",
			bytes_sent: 0,
			completed: false,
		});
		expect(
			store
				.getPeerTransfers("host-items")?.[0]
				?.items?.find((item) => item.item_id === "other-file"),
		).toBeUndefined();
	} finally {
		mock.restore();
	}
});
