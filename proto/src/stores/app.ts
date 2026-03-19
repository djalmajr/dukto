import { createSignal } from "solid-js";
import { createStore } from "solid-js/store";

// --- Theme ---
export type ThemeMode = "light" | "dark" | "system";

const [theme, setTheme] = createSignal<ThemeMode>(
	(localStorage.getItem("dukto-proto-theme") as ThemeMode) || "light",
);

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
	localStorage.setItem("dukto-proto-theme", next);
}

export { theme, setTheme };

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

// Re-export sortFileItems as sortFiles for convenience
export { sortFileItems as sortFiles } from "@app/lib/format";

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

// --- App flow state ---
export type AppScreen =
	| { id: "idle" }
	| { id: "drag-over" }
	| { id: "preview"; peer: PeerInfo; files: FileItem[] }
	| { id: "sending"; peer: PeerInfo; percent: number }
	| { id: "incoming"; from: PeerInfo; itemCount: number; totalSize: number }
	| { id: "receiving"; from: PeerInfo; percent: number }
	| { id: "complete"; direction: "send" | "receive"; peer: PeerInfo }
	| { id: "error"; message: string };

const [screen, setScreen] = createSignal<AppScreen>({ id: "idle" });

export { screen, setScreen, MOCK_FILES, MOCK_PEERS };

// --- Settings ---
const [destinationDir] = createSignal("/Users/djalmajr/Downloads");
export { destinationDir };
