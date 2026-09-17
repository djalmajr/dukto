import { listen } from "@tauri-apps/api/event";
import { createStore, produce } from "solid-js/store";
import { cancelTransfer as cancelNativeTransfer, respondTransfer } from "~/helpers/tauri";
import type { TransferSlot } from "~/routes/-components/peers/peer-card";
import type { PeerIdentity } from "~/stores/peers";
import { formatBytes } from "~/utils/format";

export interface TransferProgress {
	transfer_id: string;
	bytes_sent: number;
	bytes_total: number;
	speed_bps: number;
	percent: number;
}

export interface IncomingRequest {
	transfer_id: string;
	sender_device_id: string;
	sender?: PeerIdentity | null;
	item_count: number;
	total_size: number;
}

export type TransferStatus =
	| "waiting_approval"
	| "sending"
	| "receiving"
	| "complete"
	| "error"
	| "rejected";

export interface ActiveTransfer {
	transfer_id: string;
	peer_device_id: string;
	peer?: PeerIdentity;
	direction: "send" | "receive";
	status: TransferStatus;
	percent: number;
	bytes_sent: number;
	bytes_total: number;
	speed_bps?: number;
	error?: string;
	completed_at?: number;
}

const [transfers, setTransfers] = createStore<Record<string, ActiveTransfer>>({});
const [incomingRequest, setIncomingRequest] = createStore<{
	current: IncomingRequest | null;
	queue: IncomingRequest[];
}>({ current: null, queue: [] });
const respondingTo = new Set<string>();
const retiredTransferIds = new Set<string>();
const MAX_RETIRED_TRANSFER_IDS = 1024;

function retireTransferId(transferId: string) {
	retiredTransferIds.add(transferId);
	if (retiredTransferIds.size > MAX_RETIRED_TRANSFER_IDS) {
		const oldest = retiredTransferIds.values().next().value;
		if (oldest) retiredTransferIds.delete(oldest);
	}
}

function formatTransferSpeed(speedBps?: number) {
	if (!speedBps || speedBps <= 0) return undefined;
	return `${formatBytes(speedBps)}/s`;
}

function toTransferSlot(transfer: ActiveTransfer): TransferSlot {
	return {
		id: transfer.transfer_id,
		direction: transfer.direction,
		status:
			transfer.status === "complete"
				? "complete"
				: transfer.status === "error" || transfer.status === "rejected"
					? "error"
					: transfer.status === "waiting_approval"
						? "waiting_approval"
						: "active",
		percent: transfer.percent,
		bytesSent: transfer.bytes_sent,
		bytesTotal: transfer.bytes_total,
		speed: formatTransferSpeed(transfer.speed_bps),
		errorMsg: transfer.error,
		completedAt: transfer.completed_at,
	};
}

function getPeerTransfers(peerId: string): TransferSlot[] | undefined {
	const peerTransfers = Object.values(transfers).filter(
		(transfer) => transfer.peer_device_id === peerId,
	);
	if (peerTransfers.length === 0) return undefined;
	return peerTransfers.map(toTransferSlot);
}

function removeIncomingRequest(transferId: string) {
	const current = incomingRequest.current;
	const queue = incomingRequest.queue;
	if (current?.transfer_id === transferId) {
		setIncomingRequest({ current: queue[0] ?? null, queue: queue.slice(1) });
		return;
	}
	if (queue.some((request) => request.transfer_id === transferId)) {
		setIncomingRequest(
			"queue",
			queue.filter((request) => request.transfer_id !== transferId),
		);
	}
}

function hasIncomingRequest(transferId: string) {
	return (
		incomingRequest.current?.transfer_id === transferId ||
		incomingRequest.queue.some((request) => request.transfer_id === transferId)
	);
}

function findIncomingRequest(transferId: string) {
	if (incomingRequest.current?.transfer_id === transferId) return incomingRequest.current;
	return incomingRequest.queue.find((request) => request.transfer_id === transferId);
}

function isTerminal(status?: TransferStatus) {
	return status === "complete" || status === "error" || status === "rejected";
}

function startOrMergeSendTransfer(
	transferId: string,
	peerDeviceId: string,
	bytesTotal?: number,
	peer?: PeerIdentity,
) {
	if (retiredTransferIds.has(transferId)) return;
	setTransfers(transferId, (prev) => {
		const status = isTerminal(prev?.status)
			? prev.status
			: prev?.status === "sending"
				? "sending"
				: "waiting_approval";
		return {
			transfer_id: transferId,
			peer_device_id: peerDeviceId || prev?.peer_device_id || "",
			peer: peer ?? prev?.peer,
			direction: "send",
			status,
			percent: prev?.percent ?? 0,
			bytes_sent: prev?.bytes_sent ?? 0,
			bytes_total: prev && prev.bytes_total > 0 ? prev.bytes_total : (bytesTotal ?? 0),
			speed_bps: prev?.speed_bps,
			error: prev?.error,
			completed_at: prev?.completed_at,
		};
	});
}

function startOrMergeReceiveTransfer(request: IncomingRequest) {
	if (retiredTransferIds.has(request.transfer_id)) return;
	setTransfers(request.transfer_id, (prev) => ({
		transfer_id: request.transfer_id,
		peer_device_id: request.sender_device_id,
		peer: getIncomingPeerIdentity(request),
		direction: "receive",
		status: isTerminal(prev?.status) ? prev.status : "receiving",
		percent: prev?.percent ?? 0,
		bytes_sent: prev?.bytes_sent ?? 0,
		bytes_total: prev && prev.bytes_total > 0 ? prev.bytes_total : request.total_size,
		speed_bps: prev?.speed_bps,
		error: prev?.error,
		completed_at: prev?.completed_at,
	}));
}

function getIncomingPeerIdentity(request: IncomingRequest): PeerIdentity {
	if (request.sender?.device_id === request.sender_device_id) return request.sender;
	return {
		device_id: request.sender_device_id,
		display_name: request.sender_device_id.slice(0, 8),
		hostname: "",
		platform: "unknown",
	};
}

function getUndiscoveredTransferPeers(discoveredIds: Iterable<string>): PeerIdentity[] {
	const discovered = new Set(discoveredIds);
	const result = new Map<string, PeerIdentity>();
	for (const transfer of Object.values(transfers)) {
		if (transfer.peer && !discovered.has(transfer.peer_device_id)) {
			result.set(transfer.peer_device_id, transfer.peer);
		}
	}
	return [...result.values()];
}

function clearTransfer(transferId: string) {
	retireTransferId(transferId);
	setTransfers(
		produce((state) => {
			delete state[transferId];
		}),
	);
}

function dismissPeerTransfer(transferId: string) {
	clearTransfer(transferId);
}

// Register start before progress so a fast transfer is attributed to its sender
// before a progress event needs to create a fallback record.
listen<{ transfer_id: string; peer_device_id: string }>("transfer:send-started", (event) => {
	if (retiredTransferIds.has(event.payload.transfer_id)) return;
	startOrMergeSendTransfer(event.payload.transfer_id, event.payload.peer_device_id);
});

listen<{ transfer_id: string }>("transfer:send-accepted", (event) => {
	if (retiredTransferIds.has(event.payload.transfer_id)) return;
	setTransfers(event.payload.transfer_id, (prev) =>
		prev && !isTerminal(prev.status)
			? {
					...prev,
					status: "sending",
				}
			: prev,
	);
});

listen<TransferProgress>("transfer:progress", (event) => {
	const progress = event.payload;
	if (retiredTransferIds.has(progress.transfer_id)) return;
	const request = findIncomingRequest(progress.transfer_id);
	setTransfers(progress.transfer_id, (prev) => {
		if (isTerminal(prev?.status)) return prev;
		return {
			...(prev ?? {
				transfer_id: progress.transfer_id,
				peer_device_id: request?.sender_device_id ?? "",
				peer: request ? getIncomingPeerIdentity(request) : undefined,
				direction: "receive" as const,
				status: "receiving" as const,
				percent: 0,
				bytes_sent: 0,
				bytes_total: request?.total_size ?? 0,
			}),
			status: prev?.direction === "send" ? "sending" : (prev?.status ?? "receiving"),
			percent: progress.percent,
			bytes_sent: progress.bytes_sent,
			bytes_total: progress.bytes_total,
			speed_bps: progress.speed_bps,
		};
	});
});

listen<IncomingRequest>("transfer:incoming", (event) => {
	const request = event.payload;
	if (retiredTransferIds.has(request.transfer_id)) return;
	if (hasIncomingRequest(request.transfer_id) || respondingTo.has(request.transfer_id)) return;
	if (incomingRequest.current === null) setIncomingRequest("current", request);
	else setIncomingRequest("queue", (queue) => [...queue, request]);
});

listen<{ transfer_id: string; bytes_sent: number }>("transfer:send-complete", (event) => {
	setTransfers(event.payload.transfer_id, (prev) =>
		prev && !isTerminal(prev.status)
			? {
					...prev,
					status: "complete",
					percent: 100,
					bytes_sent: event.payload.bytes_sent,
					completed_at: Date.now(),
				}
			: prev,
	);
});

listen<{ transfer_id: string; error: string }>("transfer:send-error", (event) => {
	setTransfers(event.payload.transfer_id, (prev) =>
		prev && !isTerminal(prev.status)
			? {
					...prev,
					status: "error",
					error: event.payload.error,
					completed_at: Date.now(),
				}
			: prev,
	);
});

listen<{ transfer_id: string; error: string }>("transfer:receive-error", (event) => {
	if (retiredTransferIds.has(event.payload.transfer_id)) return;
	const request = findIncomingRequest(event.payload.transfer_id);
	setTransfers(event.payload.transfer_id, (prev) => {
		if (prev) {
			if (isTerminal(prev.status)) return prev;
			return {
				...prev,
				status: "error",
				error: event.payload.error,
				completed_at: Date.now(),
			};
		}
		if (!request) return prev;
		return {
			transfer_id: event.payload.transfer_id,
			peer_device_id: request.sender_device_id,
			peer: getIncomingPeerIdentity(request),
			direction: "receive",
			status: "error",
			percent: 0,
			bytes_sent: 0,
			bytes_total: request.total_size,
			error: event.payload.error,
			completed_at: Date.now(),
		};
	});
	if (request) removeIncomingRequest(event.payload.transfer_id);
});

listen<{ transfer_id: string; items_received: number; bytes_received: number }>(
	"transfer:complete",
	(event) => {
		const transferId = event.payload.transfer_id;
		if (retiredTransferIds.has(transferId)) return;
		const request = findIncomingRequest(transferId);
		setTransfers(transferId, (prev) => {
			if (prev) {
				if (isTerminal(prev.status)) return prev;
				return {
					...prev,
					status: "complete",
					percent: 100,
					bytes_sent: event.payload.bytes_received,
					completed_at: Date.now(),
				};
			}
			if (!request) return prev;
			return {
				transfer_id: transferId,
				peer_device_id: request.sender_device_id,
				peer: getIncomingPeerIdentity(request),
				direction: "receive",
				status: "complete",
				percent: 100,
				bytes_sent: event.payload.bytes_received,
				bytes_total: request.total_size,
				completed_at: Date.now(),
			};
		});
		if (request) removeIncomingRequest(transferId);
	},
);

listen<string>("transfer:rejected", (event) => {
	const transferId = event.payload;
	removeIncomingRequest(transferId);
	setTransfers(transferId, (prev) =>
		prev && !isTerminal(prev.status)
			? {
					...prev,
					status: "rejected",
					error: "Transfer rejected or expired.",
					completed_at: Date.now(),
				}
			: prev,
	);
});

listen<string>("transfer:cancelled", (event) => {
	removeIncomingRequest(event.payload);
	clearTransfer(event.payload);
});

function startSendTransfer(
	transferId: string,
	bytesTotal: number,
	peerDeviceId: string,
	peer?: PeerIdentity,
) {
	startOrMergeSendTransfer(transferId, peerDeviceId, bytesTotal, peer);
}

async function respondToIncoming(accepted: boolean) {
	const request = incomingRequest.current;
	if (!request || respondingTo.has(request.transfer_id)) return;
	const transferId = request.transfer_id;
	respondingTo.add(transferId);
	try {
		await respondTransfer(transferId, accepted);
		if (accepted && hasIncomingRequest(transferId)) startOrMergeReceiveTransfer(request);
		removeIncomingRequest(transferId);
	} finally {
		respondingTo.delete(transferId);
	}
}

async function acceptIncoming() {
	await respondToIncoming(true);
}

async function rejectIncoming() {
	await respondToIncoming(false);
}

async function abortTransfer(transferId: string) {
	try {
		await cancelNativeTransfer(transferId);
	} catch (error) {
		console.error(`Could not cancel transfer ${transferId}`, error);
	}
}

export {
	incomingRequest,
	transfers,
	abortTransfer,
	acceptIncoming,
	clearTransfer,
	dismissPeerTransfer,
	getPeerTransfers,
	getUndiscoveredTransferPeers,
	rejectIncoming,
	startSendTransfer,
};
