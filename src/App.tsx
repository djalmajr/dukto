import { Show, createSignal } from "solid-js";
import ErrorDisplay from "./components/ErrorDisplay";
import PeerList from "./features/peers/PeerList";
import DropZone from "./features/transfers/DropZone";
import IncomingRequestDialog from "./features/transfers/IncomingRequest";
import SendPreview from "./features/transfers/SendPreview";
import type { FileItem } from "./features/transfers/SendPreview";
import TransferList from "./features/transfers/TransferProgress";
import { type FileMetadataInfo, sendToPeer } from "./lib/tauri";
import { device } from "./stores/device";
import type { PeerInfo } from "./stores/peers";
import {
	acceptIncoming,
	incomingRequest,
	rejectIncoming,
	startSendTransfer,
} from "./stores/transfers";

type SendFlowState = { step: "idle" } | { step: "preview"; peer: PeerInfo; files: FileItem[] };

function App() {
	const [sendFlow, setSendFlow] = createSignal<SendFlowState>({ step: "idle" });
	const [error, setError] = createSignal<string | null>(null);

	function handleFilesDropped(files: FileMetadataInfo[], peer?: PeerInfo) {
		if (files.length > 0 && peer) {
			setError(null);
			setSendFlow({ step: "preview", peer, files });
		}
	}

	async function handleSend() {
		const state = sendFlow();
		if (state.step === "preview") {
			const { peer, files } = state;
			const paths = files.map((f) => f.path);
			const totalSize = files.reduce((sum, f) => sum + f.size, 0);
			setSendFlow({ step: "idle" });
			try {
				const transferId = await sendToPeer(peer.device_id, paths);
				startSendTransfer(transferId, totalSize);
			} catch (e) {
				setError(String(e));
			}
		}
	}

	function handlePeerClick(peer: PeerInfo) {
		// TODO: open file picker then go to preview with this peer
		// For now just show that peer was selected
	}

	const incomingData = () => {
		const req = incomingRequest.current;
		if (!req) return null;
		return {
			sender_name: req.sender_device_id.slice(0, 8),
			item_count: req.item_count,
			total_size: req.total_size,
		};
	};

	return (
		<DropZone onFilesDropped={(files) => handleFilesDropped(files)}>
			<main class="flex min-h-screen flex-col items-center bg-zinc-50 px-4 py-8 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100">
				<div class="w-full max-w-sm text-center">
					<h1 class="text-2xl font-bold">Dukto</h1>
					<Show when={device()}>
						{(d) => (
							<p class="mt-1 text-xs text-zinc-400">
								{d().display_name} &middot; {d().hostname}
							</p>
						)}
					</Show>
				</div>
				<div class="mt-6 w-full max-w-sm space-y-4">
					<Show when={error()}>
						{(msg) => <ErrorDisplay message={msg()} onDismiss={() => setError(null)} />}
					</Show>
					<Show when={sendFlow().step === "preview"}>
						{(() => {
							const cur = () => sendFlow() as { peer: PeerInfo; files: FileItem[] };
							return (
								<SendPreview
									peer={cur().peer}
									files={cur().files}
									onConfirm={handleSend}
									onCancel={() => setSendFlow({ step: "idle" })}
									onRemoveFile={(path) => {
										const next = cur().files.filter((f) => f.path !== path);
										if (next.length === 0) {
											setSendFlow({ step: "idle" });
										} else {
											setSendFlow({ step: "preview", peer: cur().peer, files: next });
										}
									}}
								/>
							);
						})()}
					</Show>
					<TransferList />
					<PeerList onPeerSelect={handlePeerClick} />
				</div>
			</main>
			<IncomingRequestDialog
				request={incomingData()}
				onAccept={acceptIncoming}
				onReject={rejectIncoming}
			/>
		</DropZone>
	);
}

export default App;
