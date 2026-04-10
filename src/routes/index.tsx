import { createFileRoute } from "@tanstack/solid-router";
import { Show, createMemo, createSignal } from "solid-js";
import ErrorDisplay from "~/components/error-display";
import { Button } from "~/components/ui/button";
import { t } from "~/helpers/i18n";
import { type FileMetadataInfo, resolveFileMetadata, sendToPeer } from "~/helpers/tauri";
import PeerList from "~/routes/-components/peers/peer-list";
import DropZone from "~/routes/-components/transfers/drop-zone";
import IncomingRequestDialog from "~/routes/-components/transfers/incoming-request";
import SendPreview from "~/routes/-components/transfers/send-preview";
import type { FileItem } from "~/routes/-components/transfers/send-preview";
import {
	abortTransfer,
	acceptIncoming,
	dismissPeerTransfer,
	getPeerTransfers,
	incomingRequest,
	rejectIncoming,
	startSendTransfer,
} from "~/routes/-stores/transfers";
import type { PeerInfo } from "~/stores/peers";
import { peers } from "~/stores/peers";

type SendFlowState =
	| { step: "idle" }
	| { step: "pending-files"; files: FileItem[] }
	| { step: "preview"; peer: PeerInfo; files: FileItem[] };

function HomePage() {
	const [sendFlow, setSendFlow] = createSignal<SendFlowState>({ step: "idle" });
	const [error, setError] = createSignal<string | null>(null);

	const pendingFilesLabel = createMemo(() => {
		const state = sendFlow();
		if (state.step !== "pending-files") return null;
		return t("itemCount", { count: state.files.length });
	});

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

		try {
			const { open } = await import("@tauri-apps/plugin-dialog");
			const selected = await open({ multiple: true, directory: false });
			const paths = normalizeDialogSelection(selected);
			if (paths.length === 0) return;

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
			const transferId = await sendToPeer(
				peer.device_id,
				paths,
				peer.addresses?.[0],
				peer.port,
			);
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
			{(isDragOver) => (
				<div class="flex w-full flex-1 flex-col space-y-3">
					<Show when={error()}>
						{(message) => <ErrorDisplay message={message()} onDismiss={() => setError(null)} />}
					</Show>
					<Show when={pendingFilesLabel()}>
						{(label) => (
							<div class="flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2 text-xs text-muted-foreground shadow-sm">
								<span>{label()}</span>
								<Button variant="ghost" size="sm" onClick={() => setSendFlow({ step: "idle" })}>
									{t("cancel")}
								</Button>
							</div>
						)}
					</Show>
					<div class="flex flex-1 flex-col space-y-2">
						<PeerList
							dropHighlight={isDragOver()}
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
					<IncomingRequestDialog
						request={incomingData()}
						onAccept={acceptIncoming}
						onReject={rejectIncoming}
					/>
				</div>
			)}
		</DropZone>
	);
}

export const Route = createFileRoute("/")({
	component: HomePage,
	validateSearch: (search: Record<string, unknown>) => ({
		settings: search.settings === true || search.settings === "true" || undefined,
	}),
});
