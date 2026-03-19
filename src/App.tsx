import { open } from "@tauri-apps/plugin-dialog";
import { Show, createSignal } from "solid-js";
import ErrorDisplay from "./components/ErrorDisplay";
import PeerList from "./features/peers/PeerList";
import DestinationFolder from "./features/settings/DestinationFolder";
import DropZone from "./features/transfers/DropZone";
import IncomingRequestDialog from "./features/transfers/IncomingRequest";
import PeerSelector from "./features/transfers/PeerSelector";
import SendPreview from "./features/transfers/SendPreview";
import TransferList from "./features/transfers/TransferProgress";
import { formatBytes } from "./lib/format";
import { type FileMetadataInfo, resolveFileMetadata, sendToPeer } from "./lib/tauri";
import { device } from "./stores/device";
import type { PeerInfo } from "./stores/peers";
import { startSendTransfer } from "./stores/transfers";

type SendFlowState =
	| { step: "idle" }
	| { step: "preview"; files: FileMetadataInfo[] }
	| { step: "select-peer"; files: FileMetadataInfo[] };

function App() {
	const [sendFlow, setSendFlow] = createSignal<SendFlowState>({ step: "idle" });
	const [error, setError] = createSignal<string | null>(null);

	function handleFilesDropped(files: FileMetadataInfo[]) {
		if (files.length > 0) {
			setError(null);
			setSendFlow({ step: "preview", files });
		}
	}

	async function handleFilePicker() {
		const selected = await open({ multiple: true, directory: false });
		if (selected && selected.length > 0) {
			const files = await resolveFileMetadata(selected);
			handleFilesDropped(files);
		}
	}

	async function handleFolderPicker() {
		const selected = await open({ multiple: false, directory: true });
		if (selected) {
			const files = await resolveFileMetadata([selected as string]);
			handleFilesDropped(files);
		}
	}

	function handlePreviewConfirm() {
		const state = sendFlow();
		if (state.step === "preview") {
			setSendFlow({ step: "select-peer", files: state.files });
		}
	}

	async function handlePeerSelected(peer: PeerInfo) {
		const state = sendFlow();
		if (state.step === "select-peer") {
			const files = state.files;
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

	function handleCancel() {
		setSendFlow({ step: "idle" });
	}

	return (
		<DropZone onFilesDropped={handleFilesDropped}>
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
						<SendPreview
							files={(sendFlow() as { files: FileMetadataInfo[] }).files}
							onConfirm={handlePreviewConfirm}
							onCancel={handleCancel}
							onRemoveFile={(path) => {
								const state = sendFlow();
								if (state.step === "preview") {
									const next = state.files.filter((f) => f.path !== path);
									if (next.length === 0) {
										setSendFlow({ step: "idle" });
									} else {
										setSendFlow({ step: "preview", files: next });
									}
								}
							}}
						/>
					</Show>
					<TransferList />
					<PeerList onPeerSelect={handlePeerSelected} />
					<div class="flex gap-2">
						<button
							type="button"
							class="flex-1 rounded-md border border-zinc-300 px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
							onClick={handleFilePicker}
						>
							Send files...
						</button>
						<button
							type="button"
							class="flex-1 rounded-md border border-zinc-300 px-3 py-2 text-xs font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
							onClick={handleFolderPicker}
						>
							Send folder...
						</button>
					</div>
					<DestinationFolder />
				</div>
			</main>
			<Show when={sendFlow().step === "select-peer"}>
				<PeerSelector onSelect={handlePeerSelected} onCancel={handleCancel} />
			</Show>
			<IncomingRequestDialog />
		</DropZone>
	);
}

export default App;
