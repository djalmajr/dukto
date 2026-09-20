import { createFileRoute } from "@tanstack/solid-router";
import { platform } from "@tauri-apps/plugin-os";
import { Show, createEffect, createMemo, createSignal, untrack } from "solid-js";
import ErrorDisplay from "~/components/error-display";
import { t } from "~/helpers/i18n";
import { nativeErrorKey } from "~/helpers/native-error";
import {
	pickSendItems,
	supportsFolderSelection,
	supportsMediaSelection,
} from "~/helpers/send-item-picker";
import { releaseSelectedFiles, resolveFileMetadata, sendToPeer } from "~/helpers/tauri";
import PeerList from "~/routes/-components/peers/peer-list";
import DropZone from "~/routes/-components/transfers/drop-zone";
import IncomingRequestDialog from "~/routes/-components/transfers/incoming-request";
import SendPreview from "~/routes/-components/transfers/send-preview";
import { createHostSelection } from "~/routes/-helpers/host-selection";
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
import { selectPreferredIpv4 } from "~/utils/device-address";
import { formatDeviceHostname } from "~/utils/device-hostname";

function HomePage() {
	const currentPlatform = (() => {
		try {
			return platform();
		} catch {
			return "unknown";
		}
	})();
	const selection = createHostSelection(resolveFileMetadata);
	const [error, setError] = createSignal<string | null>(null);
	const [picking, setPicking] = createSignal(false);

	createEffect(() => {
		const available = Object.keys(peers);
		untrack(() => releaseFiles(selection.retainAvailable(available)));
	});

	function releaseFiles(files: Array<{ path: string }>) {
		void releaseSelectedFiles(files.map((file) => file.path)).catch((error) => {
			console.error("Could not release selected files", error);
		});
	}

	async function addPaths(hostId: string, paths: string[], revision = selection.revision(hostId)) {
		if (!peers[hostId]) return;
		try {
			const discarded = await selection.add(hostId, paths, revision);
			releaseFiles(discarded);
		} catch (e) {
			if (revision === selection.revision(hostId)) setError(String(e));
		}
	}

	async function pickItems(peer: PeerInfo, directory: boolean, media = false) {
		if (picking() || !peers[peer.device_id]) return;
		const hostId = peer.device_id;
		const revision = selection.revision(hostId);
		setPicking(true);
		try {
			const selected = await pickSendItems(currentPlatform, directory, media);
			if (selected) releaseFiles(selection.addResolved(hostId, selected, revision));
		} catch (e) {
			if (revision === selection.revision(hostId)) setError(String(e));
		} finally {
			setPicking(false);
		}
	}

	async function handleSend(hostId: string) {
		const peer = peers[hostId];
		const files = selection.files(hostId);
		if (!peer || files.length === 0) return;
		const paths = files.map((file) => file.path);
		const totalSize = files.reduce((sum, file) => sum + file.size, 0);
		try {
			const transferId = await sendToPeer(
				hostId,
				paths,
				selectPreferredIpv4(peer.addresses ?? []),
				peer.port,
			);
			selection.cancel(hostId);
			startSendTransfer(transferId, totalSize, hostId, peer);
		} catch (e) {
			setError(String(e));
		}
	}

	function cancelSelection(hostId: string) {
		releaseFiles(selection.cancel(hostId));
	}

	function removeSelection(hostId: string, path: string) {
		releaseFiles(selection.remove(hostId, path));
	}

	const incomingData = createMemo(() => {
		const request = incomingRequest.current;
		if (!request) return null;

		const sender =
			request.sender?.device_id === request.sender_device_id
				? request.sender
				: peers[request.sender_device_id];
		const senderName = sender?.display_name.trim() || request.sender_device_id.slice(0, 8);
		const senderHost = sender ? formatDeviceHostname(sender.hostname.trim()) : "";
		return {
			sender_name: senderHost ? `${senderName}@${senderHost}` : senderName,
			item_count: request.item_count,
			total_size: request.total_size,
		};
	});

	return (
		<DropZone onFilesDropped={(hostId, paths) => void addPaths(hostId, paths)}>
			{(dropTargetId) => (
				<div class="flex w-full flex-1 flex-col space-y-3">
					<Show when={error()}>
						{(message) => (
							<ErrorDisplay
								message={t(nativeErrorKey(message()))}
								onDismiss={() => setError(null)}
							/>
						)}
					</Show>
					<PeerList
						actionsDisabled={picking()}
						dropTargetId={dropTargetId()}
						onAddFiles={(peer) => void pickItems(peer, false)}
						onAddFolders={
							supportsFolderSelection(currentPlatform)
								? (peer) => void pickItems(peer, true)
								: undefined
						}
						onAddMedia={
							supportsMediaSelection(currentPlatform)
								? (peer) => void pickItems(peer, false, true)
								: undefined
						}
						getTransfers={(peerId) => getPeerTransfers(peerId)}
						getExpandedContent={(peer) => {
							if (selection.files(peer.device_id).length === 0) return undefined;
							return (
								<SendPreview
									embedded
									peer={peer}
									files={selection.files(peer.device_id)}
									onConfirm={() => handleSend(peer.device_id)}
									onCancel={() => cancelSelection(peer.device_id)}
									onRemoveFile={(path) => removeSelection(peer.device_id, path)}
								/>
							);
						}}
						onAbortTransfer={(id) => abortTransfer(id)}
						onDismissTransfer={(id) => dismissPeerTransfer(id)}
					/>
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
