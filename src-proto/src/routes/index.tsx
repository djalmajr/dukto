import { createFileRoute } from "@tanstack/solid-router";
import { For, Show, createEffect, onCleanup } from "solid-js";
import EmptyState from "~/components/empty-state";
import { t } from "~/helpers/i18n";
import PeerCard from "~/routes/-components/peers/peer-card";
import SendPreview from "~/routes/-components/transfers/send-preview";
import {
	type FileItem,
	MOCK_FILES,
	type PeerInfo,
	abortTransfer,
	dismissTransfer,
	getPeerTransfers,
	peers,
	screen,
	setScreen,
	startTransfer,
	tickAllTransfers,
	transfers,
} from "../stores/app";

function HomePage() {
	const s = screen;

	let timer: number | undefined;
	createEffect(() => {
		let hasActive = false;
		for (const id of Object.keys(transfers)) {
			const slot = transfers[id];
			if (slot?.status === "active") {
				void slot.percent;
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
				<div class="flex flex-1 flex-col space-y-2">
					<Show
						when={peers.length > 0}
						fallback={
							<div class="flex flex-1 items-center justify-center">
								<EmptyState title={t("noDevices")} description={t("noDevicesHint")} />
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
									onAbortTransfer={(id) => abortTransfer(id)}
									onDismissTransfer={(id) => dismissTransfer(id)}
								/>
							)}
						</For>
					</Show>
				</div>
			</div>
		</>
	);
}

export const Route = createFileRoute("/")({
	component: HomePage,
});
