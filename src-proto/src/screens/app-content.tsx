import { For, Show, createEffect, onCleanup } from "solid-js";
import EmptyState from "~/components/empty-state";
import { Button } from "~/components/ui/button";
import PeerCard from "~/features/peers/peer-card";
import IncomingRequestDialog from "~/features/transfers/incoming-request";
import SendPreview from "~/features/transfers/send-preview";
import {
	type FileItem,
	MOCK_FILES,
	type PeerInfo,
	clearAllTransfers,
	dismissTransfer,
	failTransfer,
	getPeerTransfers,
	hidePeers,
	peers,
	screen,
	setScreen,
	showPeers,
	startTransfer,
	tickAllTransfers,
	transfers,
} from "../stores/app";

function AppContent() {
	const s = screen;

	// Tick all active transfers
	let timer: number | undefined;
	createEffect(() => {
		// Access percent so effect re-runs on every tick
		let hasActive = false;
		for (const id of Object.keys(transfers)) {
			const p = transfers[id];
			if (p?.send?.status === "active") {
				void p.send.percent;
				hasActive = true;
			}
			if (p?.receive?.status === "active") {
				void p.receive.percent;
				hasActive = true;
			}
		}
		clearTimeout(timer);
		if (hasActive) {
			timer = window.setTimeout(() => tickAllTransfers(), 200);
		}
	});
	onCleanup(() => clearTimeout(timer));

	function handlePeerClick(peer: PeerInfo) {
		setScreen({ id: "preview", peer, files: MOCK_FILES });
	}

	function handleAcceptIncoming(from: PeerInfo, totalSize: number) {
		startTransfer(from.device_id, "receive", totalSize, "18.7 MB/s");
		setScreen({ id: "idle" });
	}

	return (
		<>
			<div class="flex w-full flex-1 flex-col space-y-3">
				{/* Peer list */}
				<div class="flex flex-1 flex-col space-y-2">
					<Show
						when={peers.length > 0}
						fallback={
							<div class="flex flex-1 items-center justify-center">
								<EmptyState
									title="No devices found"
									description="Make sure other devices are running Dukto on the same network"
								/>
							</div>
						}
					>
						<For each={peers}>
							{(peer) => (
								<PeerCard
									peer={peer}
									transfers={getPeerTransfers(peer.device_id)}
									expandedContent={
										s().id === "preview" &&
										(s() as { peer: PeerInfo }).peer.device_id === peer.device_id ? (
											<SendPreview
												embedded
												peer={(s() as { peer: PeerInfo }).peer}
												files={(s() as { files: FileItem[] }).files}
												onConfirm={() => {
													startTransfer(peer.device_id, "send", 237500000, "24.5 MB/s");
													setScreen({ id: "idle" });
												}}
												onCancel={() => setScreen({ id: "idle" })}
												onRemoveFile={(path) => {
													const files = (s() as { files: FileItem[] }).files;
													const next = files.filter((file) => file.path !== path);
													if (next.length === 0) {
														setScreen({ id: "idle" });
													} else {
														setScreen({ id: "preview", peer, files: next });
													}
												}}
											/>
										) : undefined
									}
									onClick={() => handlePeerClick(peer)}
									onDismissTransfer={(dir) => dismissTransfer(peer.device_id, dir)}
								/>
							)}
						</For>
					</Show>
				</div>
			</div>

			{/* Incoming request dialog */}
			<Show when={s().id === "incoming" && s()}>
				{(cur) => {
					const data = () => cur() as { from: PeerInfo; itemCount: number; totalSize: number };
					return (
						<IncomingRequestDialog
							request={{
								sender_name: data().from.display_name,
								item_count: data().itemCount,
								total_size: data().totalSize,
							}}
							onAccept={() => handleAcceptIncoming(data().from, data().totalSize)}
							onReject={() => setScreen({ id: "idle" })}
						/>
					);
				}}
			</Show>
		</>
	);
}

export default AppContent;
