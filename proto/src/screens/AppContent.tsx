import EmptyState from "@app/components/EmptyState";
import ErrorDisplay from "@app/components/ErrorDisplay";
import TransferBar from "@app/components/TransferBar";
import PeerCard from "@app/features/peers/PeerCard";
import SettingsModal from "@app/features/settings/SettingsModal";
import IncomingRequestDialog from "@app/features/transfers/IncomingRequest";
import SendPreview from "@app/features/transfers/SendPreview";
import { For, Show, createEffect, onCleanup } from "solid-js";
import {
	type FileItem,
	MOCK_FILES,
	type PeerInfo,
	destinationDir,
	hidePeers,
	peers,
	screen,
	setScreen,
	setTheme,
	showPeers,
	theme,
} from "../stores/app";

interface AppContentProps {
	showSettings: boolean;
	onCloseSettings: () => void;
}

function AppContent(props: AppContentProps) {
	const s = screen;

	// Simulate progress
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

	return (
		<>
			<div class="w-full space-y-3">
				{/* Error */}
				<Show when={s().id === "error"}>
					<ErrorDisplay
						message={(s() as { message: string }).message}
						onDismiss={() => setScreen({ id: "idle" })}
					/>
				</Show>

				{/* Send preview */}
				<Show when={s().id === "preview"}>
					{(() => {
						const cur = () => s() as { peer: PeerInfo; files: FileItem[] };
						return (
							<SendPreview
								peer={cur().peer}
								files={cur().files}
								onConfirm={() => setScreen({ id: "sending", peer: cur().peer, percent: 0 })}
								onCancel={() => setScreen({ id: "idle" })}
								onRemoveFile={(path) => {
									const next = cur().files.filter((f) => f.path !== path);
									if (next.length === 0) {
										setScreen({ id: "idle" });
									} else {
										setScreen({ id: "preview", peer: cur().peer, files: next });
									}
								}}
							/>
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
							}}
						>
							Incoming
						</ProtoBtn>
						<ProtoBtn
							onClick={() =>
								setScreen({
									id: "error",
									message:
										"Could not connect to iMac-Office.local. The device may be offline or unreachable.",
								})
							}
						>
							Error
						</ProtoBtn>
						<ProtoBtn onClick={() => setScreen({ id: "idle" })}>Reset</ProtoBtn>
					</div>
				</div>
			</div>

			{/* Incoming request */}
			<Show when={s().id === "incoming"}>
				<IncomingRequestDialog
					request={{
						sender_name: (s() as { from: PeerInfo }).from.display_name,
						item_count: (s() as { itemCount: number }).itemCount,
						total_size: (s() as { totalSize: number }).totalSize,
					}}
					onAccept={() =>
						setScreen({ id: "receiving", from: (s() as { from: PeerInfo }).from, percent: 0 })
					}
					onReject={() => setScreen({ id: "idle" })}
				/>
			</Show>

			{/* Settings */}
			<SettingsModal
				visible={props.showSettings}
				destinationDir={destinationDir()}
				theme={theme()}
				onChangeDestination={() => {}}
				onChangeTheme={setTheme}
				onClose={props.onCloseSettings}
			/>
		</>
	);
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
