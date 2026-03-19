import { listen } from "@tauri-apps/api/event";
import { createStore } from "solid-js/store";
import { respondTransfer } from "../lib/tauri";

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
	direction: "send" | "receive";
	status: TransferStatus;
	percent: number;
	bytes_sent: number;
	bytes_total: number;
	error?: string;
}

const [transfers, setTransfers] = createStore<Record<string, ActiveTransfer>>({});
const [incomingRequest, setIncomingRequest] = createStore<{
	current: IncomingRequest | null;
}>({ current: null });

// --- Event listeners ---

listen<TransferProgress>("transfer:progress", (event) => {
	const p = event.payload;
	setTransfers(p.transfer_id, (prev) => ({
		...(prev ?? {
			transfer_id: p.transfer_id,
			direction: "receive" as const,
			status: "receiving" as const,
		}),
		percent: p.percent,
		bytes_sent: p.bytes_sent,
		bytes_total: p.bytes_total,
	}));
});

listen<IncomingRequest>("transfer:incoming", (event) => {
	setIncomingRequest("current", event.payload);
});

listen<{ transfer_id: string; bytes_sent: number }>("transfer:send-complete", (event) => {
	setTransfers(event.payload.transfer_id, (prev) =>
		prev ? { ...prev, status: "complete", percent: 100 } : prev,
	);
});

listen<{ transfer_id: string; error: string }>("transfer:send-error", (event) => {
	setTransfers(event.payload.transfer_id, (prev) =>
		prev ? { ...prev, status: "error", error: event.payload.error } : prev,
	);
});

listen<{ transfer_id: string; items_received: number; bytes_received: number }>(
	"transfer:complete",
	(event) => {
		setTransfers(event.payload.transfer_id, (prev) =>
			prev ? { ...prev, status: "complete", percent: 100 } : prev,
		);
	},
);

listen<string>("transfer:rejected", (event) => {
	setTransfers(event.payload, (prev) => (prev ? { ...prev, status: "rejected" } : prev));
});

// --- Actions ---

function startSendTransfer(transferId: string, bytesTotal: number) {
	setTransfers(transferId, {
		transfer_id: transferId,
		direction: "send",
		status: "sending",
		percent: 0,
		bytes_sent: 0,
		bytes_total: bytesTotal,
	});
}

async function acceptIncoming() {
	const req = incomingRequest.current;
	if (!req) return;
	setTransfers(req.transfer_id, {
		transfer_id: req.transfer_id,
		direction: "receive",
		status: "receiving",
		percent: 0,
		bytes_sent: 0,
		bytes_total: req.total_size,
	});
	setIncomingRequest("current", null);
	await respondTransfer(req.transfer_id, true);
}

async function rejectIncoming() {
	const req = incomingRequest.current;
	if (!req) return;
	setIncomingRequest("current", null);
	await respondTransfer(req.transfer_id, false);
}

function clearTransfer(transferId: string) {
	setTransfers((prev) => {
		const next = { ...prev };
		delete next[transferId];
		return next;
	});
}

export {
	transfers,
	incomingRequest,
	startSendTransfer,
	acceptIncoming,
	rejectIncoming,
	clearTransfer,
};
