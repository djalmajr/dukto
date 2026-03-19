import { createSignal } from "solid-js";
import { createStore, produce } from "solid-js/store";

// --- Theme ---
export type ThemeMode = "light" | "dark" | "system";

const [theme, setThemeSignal] = createSignal<ThemeMode>(
	(localStorage.getItem("dukto-proto-theme") as ThemeMode) || "light",
);

export function setTheme(next: ThemeMode) {
	setThemeSignal(next);
	localStorage.setItem("dukto-proto-theme", next);
}

export function resolvedTheme(): "light" | "dark" {
	const t = theme();
	if (t === "system") {
		return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
	}
	return t;
}

export function cycleTheme() {
	const order: ThemeMode[] = ["light", "dark", "system"];
	const next = order[(order.indexOf(theme()) + 1) % order.length];
	setTheme(next);
}

export { theme };

// --- Peers ---
export interface PeerInfo {
	device_id: string;
	display_name: string;
	hostname: string;
	platform: "macos" | "windows" | "linux";
}

const MOCK_PEERS: PeerInfo[] = [
	{ device_id: "p1", display_name: "djalmajr", hostname: "iMac-Office.local", platform: "macos" },
	{ device_id: "p2", display_name: "djalma", hostname: "DESKTOP-WIN11.local", platform: "windows" },
	{ device_id: "p3", display_name: "djalma", hostname: "ubuntu-server.local", platform: "linux" },
];

const [peers, setPeers] = createStore<PeerInfo[]>([]);

export function showPeers() {
	setPeers(MOCK_PEERS);
}
export function hidePeers() {
	setPeers([]);
}
export { peers };

// --- Files ---
export interface FileItem {
	name: string;
	path: string;
	size: number;
	is_dir: boolean;
}

export { sortFileItems as sortFiles } from "~/lib/format";

const MOCK_FILES: FileItem[] = [
	{ name: "project-files", path: "/Users/djalmajr/project-files", size: 8200000, is_dir: true },
	{
		name: "report-final.pdf",
		path: "/Users/djalmajr/report-final.pdf",
		size: 3100000,
		is_dir: false,
	},
	{
		name: "screenshot-2026-03-19.png",
		path: "/Users/djalmajr/screenshot.png",
		size: 1200000,
		is_dir: false,
	},
	{ name: "notes.txt", path: "/Users/djalmajr/notes.txt", size: 340, is_dir: false },
	{ name: "backup-2026-03.zip", path: "/Users/djalmajr/backup.zip", size: 45000000, is_dir: false },
	{
		name: "presentation.key",
		path: "/Users/djalmajr/presentation.key",
		size: 12400000,
		is_dir: false,
	},
	{ name: "design-assets", path: "/Users/djalmajr/design-assets", size: 34000000, is_dir: true },
	{
		name: "video-demo.mp4",
		path: "/Users/djalmajr/video-demo.mp4",
		size: 128000000,
		is_dir: false,
	},
	{ name: "config.yaml", path: "/Users/djalmajr/config.yaml", size: 892, is_dir: false },
	{
		name: "database-export.sql",
		path: "/Users/djalmajr/database-export.sql",
		size: 5600000,
		is_dir: false,
	},
];

// --- Per-peer transfers (supports simultaneous send + receive per peer) ---
export interface TransferSlot {
	status: "active" | "complete" | "error";
	percent: number;
	bytesSent: number;
	bytesTotal: number;
	speed?: string;
	errorMsg?: string;
	completedAt?: number;
}

export interface PeerTransfers {
	send?: TransferSlot;
	receive?: TransferSlot;
}

const [transfers, setTransfers] = createStore<Record<string, PeerTransfers>>({});

export function startTransfer(
	peerId: string,
	direction: "send" | "receive",
	bytesTotal: number,
	speed?: string,
) {
	if (!transfers[peerId]) {
		setTransfers(peerId, {});
	}
	setTransfers(peerId, direction, {
		status: "active",
		percent: 0,
		bytesSent: 0,
		bytesTotal,
		speed,
	});
}

export function tickAllTransfers() {
	setTransfers(
		produce((all) => {
			for (const peerId of Object.keys(all)) {
				const peer = all[peerId];
				for (const dir of ["send", "receive"] as const) {
					const slot = peer[dir];
					if (!slot || slot.status !== "active") continue;
					const inc = dir === "send" ? 8 : 6;
					const next = Math.min(slot.percent + inc, 100);
					slot.percent = next;
					slot.bytesSent = Math.floor((slot.bytesTotal * next) / 100);
					if (next >= 100) {
						slot.status = "complete";
						slot.completedAt = Date.now();
					}
				}
			}
		}),
	);
}

export function dismissTransfer(peerId: string, direction: "send" | "receive") {
	setTransfers(
		produce((all) => {
			const peer = all[peerId];
			if (!peer) return;
			delete peer[direction];
			if (!peer.send && !peer.receive) delete all[peerId];
		}),
	);
}

export function failTransfer(peerId: string, direction: "send" | "receive", errorMsg: string) {
	if (!transfers[peerId]) {
		setTransfers(peerId, {});
	}
	setTransfers(peerId, direction, {
		status: "error",
		percent: 0,
		bytesSent: 0,
		bytesTotal: 0,
		errorMsg,
	});
}

export function clearAllTransfers() {
	setTransfers({});
}

export function getPeerTransfers(peerId: string): PeerTransfers | undefined {
	return transfers[peerId];
}

export { transfers };

// --- UI flow (non-transfer screens) ---
export type AppScreen =
	| { id: "idle" }
	| { id: "preview"; peer: PeerInfo; files: FileItem[] }
	| { id: "incoming"; from: PeerInfo; itemCount: number; totalSize: number };

const [screen, setScreen] = createSignal<AppScreen>({ id: "idle" });

export { screen, setScreen, MOCK_FILES, MOCK_PEERS };

// --- Settings ---
const [destinationDir] = createSignal("/Users/djalmajr/Downloads");
export { destinationDir };
