import Button from "@app/components/Button";
import EmptyState from "@app/components/EmptyState";
import ErrorDisplay from "@app/components/ErrorDisplay";
import Icon from "@app/components/Icon";
import TransferBar from "@app/components/TransferBar";
import PeerCard from "@app/features/peers/PeerCard";
import { formatBytes } from "@app/lib/format";
import { For, Show, createEffect, onCleanup } from "solid-js";
import {
	type FileItem,
	MOCK_FILES,
	type PeerInfo,
	type ThemeMode,
	destinationDir,
	hidePeers,
	peers,
	screen,
	setScreen,
	setTheme,
	showPeers,
	sortFiles,
	theme,
} from "../stores/app";

interface AppContentProps {
	showSettings: boolean;
	onCloseSettings: () => void;
}

function AppContent(props: AppContentProps) {
	const s = screen;

	// Simulate sending progress
	let timer: number | undefined;
	createEffect(() => {
		const cur = s();
		if (cur.id === "sending") {
			if (cur.percent < 100) {
				timer = window.setTimeout(() => {
					setScreen({ ...cur, percent: Math.min(cur.percent + 8, 100) });
				}, 200);
			} else {
				setScreen({ id: "complete", direction: "send", peer: cur.peer });
			}
		}
		if (cur.id === "receiving") {
			if (cur.percent < 100) {
				timer = window.setTimeout(() => {
					setScreen({ ...cur, percent: Math.min(cur.percent + 6, 100) });
				}, 200);
			} else {
				setScreen({ id: "complete", direction: "receive", peer: cur.from });
			}
		}
	});
	onCleanup(() => clearTimeout(timer));

	function handlePeerClick(peer: PeerInfo) {
		setScreen({ id: "preview", peer, files: MOCK_FILES });
	}

	function simulateIncoming() {
		setScreen({
			id: "incoming",
			from: peers[0] || {
				device_id: "x",
				display_name: "Unknown",
				hostname: "unknown.local",
				platform: "macos",
			},
			itemCount: 4,
			totalSize: 12800000,
		});
	}

	function simulateError() {
		setScreen({
			id: "error",
			message: "Could not connect to iMac-Office.local. The device may be offline or unreachable.",
		});
	}

	return (
		<>
			<div class="w-full space-y-3">
				{/* Error banner */}
				<Show when={s().id === "error"}>
					<ErrorDisplay
						message={(s() as { message: string }).message}
						onDismiss={() => setScreen({ id: "idle" })}
					/>
				</Show>

				{/* Send preview — peer as header */}
				<Show when={s().id === "preview"}>
					{(() => {
						const cur = () => s() as { peer: PeerInfo; files: FileItem[] };
						const sorted = () => sortFiles(cur().files);
						const removeFile = (path: string) => {
							const next = cur().files.filter((f) => f.path !== path);
							if (next.length === 0) {
								setScreen({ id: "idle" });
							} else {
								setScreen({ id: "preview", peer: cur().peer, files: next });
							}
						};
						return (
							<div class="rounded-lg border border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
								{/* Peer header */}
								<div class="flex items-center gap-3 border-b border-zinc-100 px-4 py-3 dark:border-zinc-800">
									<span class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-100 text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400">
										<Icon name={platformIcon(cur().peer.platform)} size={18} />
									</span>
									<div class="min-w-0 flex-1">
										<p class="text-sm font-medium">Send to {cur().peer.display_name}</p>
										<p class="text-[11px] text-zinc-500">{cur().peer.hostname}</p>
									</div>
									<button
										class="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
										onClick={() => setScreen({ id: "idle" })}
									>
										<Icon name="mdi:close" size={16} />
									</button>
								</div>
								{/* File list */}
								<div class="px-4 py-3">
									<p class="text-xs text-zinc-500">
										{sorted().length} {sorted().length === 1 ? "item" : "items"} &middot;{" "}
										{formatBytes(sorted().reduce((sum, f) => sum + f.size, 0))}
									</p>
									<ul class="mt-2 max-h-60 space-y-0.5 overflow-y-auto">
										<For each={sorted()}>
											{(f) => (
												<li class="group flex items-center gap-2 rounded px-1 py-0.5 text-xs hover:bg-zinc-50 dark:hover:bg-zinc-800">
													<Icon
														name={f.is_dir ? "mdi:folder-outline" : "mdi:file-outline"}
														size={14}
														class="shrink-0 text-zinc-400"
													/>
													<span class="min-w-0 flex-1 truncate">{f.name}</span>
													<span class="shrink-0 text-zinc-400">{formatBytes(f.size)}</span>
													<button
														type="button"
														class="ml-1 shrink-0 rounded p-0.5 text-zinc-300 opacity-0 transition-opacity hover:text-zinc-500 group-hover:opacity-100 dark:text-zinc-600 dark:hover:text-zinc-400"
														onClick={() => removeFile(f.path)}
														title="Remove"
													>
														<Icon name="mdi:close" size={12} />
													</button>
												</li>
											)}
										</For>
									</ul>
									<div class="mt-3 flex gap-2">
										<Button
											variant="primary"
											class="flex-1"
											onClick={() => setScreen({ id: "sending", peer: cur().peer, percent: 0 })}
										>
											Send
										</Button>
										<Button class="flex-1" onClick={() => setScreen({ id: "idle" })}>
											Cancel
										</Button>
									</div>
								</div>
							</div>
						);
					})()}
				</Show>

				{/* Transfer progress */}
				<Show when={s().id === "sending"}>
					<TransferBar
						direction="send"
						status="active"
						percent={(s() as { percent: number }).percent}
						bytesSent={Math.floor((12800000 * (s() as { percent: number }).percent) / 100)}
						bytesTotal={12800000}
						peerName={(s() as { peer: PeerInfo }).peer.hostname}
						speed="24.5 MB/s"
					/>
				</Show>
				<Show when={s().id === "receiving"}>
					<TransferBar
						direction="receive"
						status="active"
						percent={(s() as { percent: number }).percent}
						bytesSent={Math.floor((12800000 * (s() as { percent: number }).percent) / 100)}
						bytesTotal={12800000}
						peerName={(s() as { from: PeerInfo }).from.hostname}
						speed="18.7 MB/s"
					/>
				</Show>
				<Show when={s().id === "complete"}>
					<TransferBar
						direction={(s() as { direction: "send" | "receive" }).direction}
						status="complete"
						percent={100}
						bytesSent={12800000}
						bytesTotal={12800000}
						peerName={(s() as { peer: PeerInfo }).peer.hostname}
						onDismiss={() => setScreen({ id: "idle" })}
					/>
				</Show>

				{/* Peer list */}
				<div class="space-y-2">
					<Show
						when={peers.length > 0}
						fallback={
							<EmptyState
								title="No devices found"
								description="Make sure other devices are running Dukto on the same network"
							/>
						}
					>
						<For each={peers}>
							{(peer) => (
								<PeerCard
									peer={peer}
									status={
										s().id === "sending" &&
										(s() as { peer: PeerInfo }).peer.device_id === peer.device_id
											? "Sending"
											: s().id === "receiving" &&
													(s() as { from: PeerInfo }).from.device_id === peer.device_id
												? "Receiving"
												: undefined
									}
									onClick={() => handlePeerClick(peer)}
								/>
							)}
						</For>
					</Show>
				</div>

				{/* Proto controls */}
				<div class="rounded-lg border-2 border-dashed border-zinc-300 p-3 space-y-2 dark:border-zinc-700">
					<p class="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
						Proto controls
					</p>
					<div class="flex flex-wrap gap-1">
						<ProtoBtn
							onClick={() => {
								hidePeers();
								setScreen({ id: "idle" });
							}}
						>
							No peers
						</ProtoBtn>
						<ProtoBtn
							onClick={() => {
								showPeers();
								setScreen({ id: "idle" });
							}}
						>
							Show peers
						</ProtoBtn>
						<ProtoBtn
							onClick={() => {
								showPeers();
								handlePeerClick(peers[0]);
							}}
						>
							Send to peer
						</ProtoBtn>
						<ProtoBtn
							onClick={() => {
								showPeers();
								simulateIncoming();
							}}
						>
							Incoming
						</ProtoBtn>
						<ProtoBtn onClick={simulateError}>Error</ProtoBtn>
						<ProtoBtn onClick={() => setScreen({ id: "idle" })}>Reset</ProtoBtn>
					</div>
				</div>
			</div>

			{/* Incoming request modal */}
			<Show when={s().id === "incoming"}>
				<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
					<div class="w-full max-w-xs rounded-lg bg-white p-4 shadow-xl dark:bg-zinc-900">
						<h3 class="text-sm font-semibold">Incoming transfer</h3>
						<p class="mt-2 text-xs text-zinc-500">
							{(s() as { itemCount: number }).itemCount} items &middot;{" "}
							{formatBytes((s() as { totalSize: number }).totalSize)}
						</p>
						<p class="mt-1 text-xs text-zinc-400">
							From: {(s() as { from: PeerInfo }).from.display_name}
						</p>
						<div class="mt-3 flex gap-2">
							<Button
								variant="primary"
								class="flex-1"
								onClick={() =>
									setScreen({ id: "receiving", from: (s() as { from: PeerInfo }).from, percent: 0 })
								}
							>
								Accept
							</Button>
							<Button class="flex-1" onClick={() => setScreen({ id: "idle" })}>
								Reject
							</Button>
						</div>
					</div>
				</div>
			</Show>

			{/* Settings modal */}
			<Show when={props.showSettings}>
				<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
					<div class="w-full max-w-xs rounded-lg bg-white p-4 shadow-xl dark:bg-zinc-900">
						<h3 class="text-sm font-semibold">Settings</h3>
						<div class="mt-4 space-y-4">
							<div class="space-y-1">
								<p class="text-[11px] font-medium text-zinc-500">Save files to</p>
								<div class="flex items-center gap-2">
									<span class="min-w-0 flex-1 truncate rounded-md border border-zinc-200 bg-zinc-100 px-2 py-1.5 text-[11px] text-zinc-700 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300">
										{destinationDir()}
									</span>
									<Button>Change</Button>
								</div>
							</div>
							<div class="space-y-1">
								<p class="text-[11px] font-medium text-zinc-500">Appearance</p>
								<div class="flex gap-1">
									<For each={["light", "dark", "system"] as ThemeMode[]}>
										{(opt) => (
											<button
												class="flex-1 rounded-md border px-2 py-1.5 text-[11px] font-medium capitalize transition-colors"
												classList={{
													"border-blue-500 bg-blue-50 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400":
														theme() === opt,
													"border-zinc-300 text-zinc-500 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-400 dark:hover:bg-zinc-800":
														theme() !== opt,
												}}
												onClick={() => setTheme(opt)}
											>
												{opt}
											</button>
										)}
									</For>
								</div>
							</div>
						</div>
						<Button class="mt-5 w-full" onClick={props.onCloseSettings}>
							Done
						</Button>
					</div>
				</div>
			</Show>
		</>
	);
}

function platformIcon(platform: string): string {
	const icons: Record<string, string> = {
		macos: "ic:baseline-apple",
		windows: "mdi:microsoft-windows",
		linux: "cib:linux",
	};
	return icons[platform] ?? "mdi:monitor";
}

function ProtoBtn(props: { children: string; onClick: () => void }) {
	return (
		<button
			class="rounded bg-zinc-200 px-2 py-1 text-[10px] font-medium text-zinc-600 transition-colors hover:bg-zinc-300 dark:bg-zinc-800 dark:text-zinc-300 dark:hover:bg-zinc-700"
			onClick={props.onClick}
		>
			{props.children}
		</button>
	);
}

export default AppContent;
