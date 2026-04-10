import { listen } from "@tauri-apps/api/event";
import { createStore, produce } from "solid-js/store";
import { respondTransfer } from "~/helpers/tauri";
import type { TransferSlot } from "~/routes/-components/peers/peer-card";
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
	item_count: number;
	total_size: number;
}

export type TransferStatus = "sending" | "receiving" | "complete" | "error" | "rejected";

export interface ActiveTransfer {
	transfer_id: string;
	peer_device_id: string;
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
}>({ current: null });

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

function dismissPeerTransfer(transferId: string) {
	clearTransfer(transferId);
}

listen<TransferProgress>("transfer:progress", (event) => {
	const progress = event.payload;
	setTransfers(progress.transfer_id, (prev) => ({
		...(prev ?? {
			transfer_id: progress.transfer_id,
			peer_device_id: "",
			direction: "receive" as const,
			status: "receiving" as const,
		}),
		percent: progress.percent,
		bytes_sent: progress.bytes_sent,
		bytes_total: progress.bytes_total,
		speed_bps: progress.speed_bps,
	}));
});

listen<IncomingRequest>("transfer:incoming", (event) => {
	setIncomingRequest("current", event.payload);
});

listen<{ transfer_id: string; bytes_sent: number }>("transfer:send-complete", (event) => {
	setTransfers(event.payload.transfer_id, (prev) =>
		prev
			? {
					...prev,
					status: "complete",
					percent: 100,
					completed_at: Date.now(),
				}
			: prev,
	);
});

listen<{ transfer_id: string; error: string }>("transfer:send-error", (event) => {
	setTransfers(event.payload.transfer_id, (prev) =>
		prev
			? {
					...prev,
					status: "error",
					error: event.payload.error,
					completed_at: Date.now(),
				}
			: prev,
	);
});

listen<{ transfer_id: string; items_received: number; bytes_received: number }>(
	"transfer:complete",
	(event) => {
		setTransfers(event.payload.transfer_id, (prev) =>
			prev
				? {
						...prev,
						status: "complete",
						percent: 100,
						bytes_sent: event.payload.bytes_received,
						completed_at: Date.now(),
					}
				: prev,
		);
	},
);

listen<string>("transfer:rejected", (event) => {
	setTransfers(event.payload, (prev) =>
		prev
			? {
					...prev,
					status: "rejected",
					error: "Transfer rejected.",
					completed_at: Date.now(),
				}
			: prev,
	);
});

function startSendTransfer(transferId: string, bytesTotal: number, peerDeviceId: string) {
	setTransfers(transferId, {
		transfer_id: transferId,
		peer_device_id: peerDeviceId,
		direction: "send",
		status: "sending",
		percent: 0,
		bytes_sent: 0,
		bytes_total: bytesTotal,
	});
}

async function acceptIncoming() {
	const request = incomingRequest.current;
	if (!request) return;
	setTransfers(request.transfer_id, {
		transfer_id: request.transfer_id,
		peer_device_id: request.sender_device_id,
		direction: "receive",
		status: "receiving",
		percent: 0,
		bytes_sent: 0,
		bytes_total: request.total_size,
	});
	setIncomingRequest("current", null);
	await respondTransfer(request.transfer_id, true);
}

async function rejectIncoming() {
	const request = incomingRequest.current;
	if (!request) return;
	setIncomingRequest("current", null);
	await respondTransfer(request.transfer_id, false);
}

function clearTransfer(transferId: string) {
	setTransfers(
		produce((state) => {
			delete state[transferId];
		}),
	);
}

function abortTransfer(transferId: string) {
	clearTransfer(transferId);
}

export {
	incomingRequest,
	transfers,
	abortTransfer,
	acceptIncoming,
	clearTransfer,
	dismissPeerTransfer,
	getPeerTransfers,
	rejectIncoming,
	startSendTransfer,
};
