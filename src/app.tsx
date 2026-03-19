import { open } from "@tauri-apps/plugin-dialog";
import { Show, createEffect, createMemo, createSignal } from "solid-js";
import ErrorDisplay from "~/components/error-display";
import Icon from "~/components/icon";
import { Button } from "~/components/ui/button";
import PeerList from "~/features/peers/peer-list";
import SettingsModal from "~/features/settings/settings-modal";
import DropZone from "~/features/transfers/drop-zone";
import IncomingRequestDialog from "~/features/transfers/incoming-request";
import SendPreview from "~/features/transfers/send-preview";
import type { FileItem } from "~/features/transfers/send-preview";
import { type FileMetadataInfo, resolveFileMetadata, sendToPeer } from "~/lib/tauri";
import { device } from "~/stores/device";
import type { PeerInfo } from "~/stores/peers";
import { peers } from "~/stores/peers";
import { resolvedTheme, setDestinationDir, setTheme, settings, theme } from "~/stores/settings";
import {
	abortTransfer,
	acceptIncoming,
	dismissPeerTransfer,
	getPeerTransfers,
	incomingRequest,
	rejectIncoming,
	startSendTransfer,
} from "~/stores/transfers";

type SendFlowState =
	| { step: "idle" }
	| { step: "pending-files"; files: FileItem[] }
	| { step: "preview"; peer: PeerInfo; files: FileItem[] };

function App() {
	const [sendFlow, setSendFlow] = createSignal<SendFlowState>({ step: "idle" });
	const [error, setError] = createSignal<string | null>(null);
	const [showSettings, setShowSettings] = createSignal(false);

	createEffect(() => {
		const dark = resolvedTheme() === "dark";
		document.documentElement.classList.toggle("dark", dark);
		document.documentElement.setAttribute("data-kb-theme", dark ? "dark" : "light");
	});

	const pendingFilesLabel = createMemo(() => {
		const state = sendFlow();
		if (state.step !== "pending-files") return null;
		return state.files.length === 1
			? "1 item ready to send"
			: `${state.files.length} items ready to send`;
	});

	async function handleChangeDestination() {
		const selected = await open({ directory: true, multiple: false });
		if (selected) {
			await setDestinationDir(selected as string);
		}
	}

	function handleFilesDropped(files: FileMetadataInfo[]) {
		if (files.length > 0) {
			setError(null);
			setSendFlow({ step: "pending-files", files });
		}
	}

	function normalizeDialogSelection(selection: string | string[] | null): string[] {
		if (!selection) return [];
		return Array.isArray(selection) ? selection : [selection];
	}

	async function handlePeerClick(peer: PeerInfo) {
		const state = sendFlow();
		if (state.step === "pending-files") {
			setSendFlow({ step: "preview", peer, files: state.files });
			return;
		}

		const selected = await open({ multiple: true, directory: false });
		const paths = normalizeDialogSelection(selected);
		if (paths.length === 0) return;

		try {
			const files = await resolveFileMetadata(paths);
			if (files.length > 0) {
				setError(null);
				setSendFlow({ step: "preview", peer, files });
			}
		} catch (e) {
			setError(String(e));
		}
	}

	async function handleSend() {
		const state = sendFlow();
		if (state.step !== "preview") return;

		const { peer, files } = state;
		const paths = files.map((file) => file.path);
		const totalSize = files.reduce((sum, file) => sum + file.size, 0);
		setSendFlow({ step: "idle" });

		try {
			const transferId = await sendToPeer(peer.device_id, paths);
			startSendTransfer(transferId, totalSize, peer.device_id);
		} catch (e) {
			setError(String(e));
		}
	}

	const incomingData = createMemo(() => {
		const request = incomingRequest.current;
		if (!request) return null;

		const sender = peers[request.sender_device_id];
		return {
			sender_name: sender?.display_name ?? request.sender_device_id.slice(0, 8),
			item_count: request.item_count,
			total_size: request.total_size,
		};
	});

	return (
		<DropZone onFilesDropped={handleFilesDropped}>
			<main class="min-h-screen bg-background px-4 py-8 text-foreground">
				<div class="mx-auto flex w-full max-w-md flex-col gap-4">
					<div class="flex items-start justify-between gap-3">
						<div class="min-w-0">
							<h1 class="text-2xl font-bold tracking-tight">Dukto</h1>
							<Show when={device()}>
								{(currentDevice) => (
									<p class="mt-1 truncate text-xs text-muted-foreground">
										{currentDevice().display_name} . {currentDevice().hostname}
									</p>
								)}
							</Show>
						</div>
						<Button variant="outline" size="icon" onClick={() => setShowSettings(true)}>
							<Icon name="mdi:cog-outline" size={16} />
						</Button>
					</div>

					<Show when={error()}>
						{(message) => <ErrorDisplay message={message()} onDismiss={() => setError(null)} />}
					</Show>

					<Show when={pendingFilesLabel()}>
						{(label) => (
							<div class="flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2 text-xs text-muted-foreground shadow-sm">
								<span>{label()}</span>
								<Button variant="ghost" size="sm" onClick={() => setSendFlow({ step: "idle" })}>
									Clear
								</Button>
							</div>
						)}
					</Show>

					<PeerList
						onPeerSelect={handlePeerClick}
						getTransfers={(peer) => getPeerTransfers(peer.device_id)}
						getExpandedContent={(peer) => {
							const state = sendFlow();
							if (state.step !== "preview" || state.peer.device_id !== peer.device_id) {
								return undefined;
							}

							return (
								<SendPreview
									embedded
									peer={state.peer}
									files={state.files}
									onConfirm={handleSend}
									onCancel={() => setSendFlow({ step: "idle" })}
									onRemoveFile={(path) => {
										const nextFiles = state.files.filter((file) => file.path !== path);
										if (nextFiles.length === 0) {
											setSendFlow({ step: "idle" });
										} else {
											setSendFlow({ step: "preview", peer: state.peer, files: nextFiles });
										}
									}}
								/>
							);
						}}
						onAbortTransfer={(id) => abortTransfer(id)}
					  onDismissTransfer={(id) => dismissPeerTransfer(id)}
					/>
				</div>
			</main>

			<IncomingRequestDialog
				request={incomingData()}
				onAccept={acceptIncoming}
				onReject={rejectIncoming}
			/>
			<SettingsModal
				open={showSettings()}
				destinationDir={settings()?.destination_dir ?? "Loading..."}
				theme={theme()}
				onChangeDestination={handleChangeDestination}
				onChangeTheme={setTheme}
				onClose={() => setShowSettings(false)}
			/>
		</DropZone>
	);
}

export default App;
