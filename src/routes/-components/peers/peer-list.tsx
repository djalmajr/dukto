import { type JSX, Show, createSignal } from "solid-js";
import EmptyState from "~/components/empty-state";
import { t } from "~/helpers/i18n";
import type { TransferSlot } from "~/routes/-components/peers/peer-card";
import PeerCard from "~/routes/-components/peers/peer-card";
import { getUndiscoveredTransferPeers } from "~/routes/-stores/transfers";
import { type PeerIdentity, type PeerInfo, peers } from "~/stores/peers";
import { formatDeviceHostname } from "~/utils/device-hostname";

import { movePeer, orderedPeerIds } from "~/routes/-helpers/peer-order";
import { savePeerOrder, settings } from "~/stores/settings";
import HostOrder from "./host-order";
import RemotePeerEntry from "./remote-peer-entry";

interface PeerListProps {
	onPeerSelect?: (peer: PeerInfo) => void;
	onAddFiles?: (peer: PeerInfo) => void;
	onAddFolders?: (peer: PeerInfo) => void;
	onAddMedia?: (peer: PeerInfo) => void;
	getExpandedContent?: (peer: PeerInfo) => JSX.Element | undefined;
	getTransfers?: (peerId: string) => TransferSlot[] | undefined;
	actionsDisabled?: boolean;
	dropTargetId?: string;
	onAbortTransfer?: (transferId: string) => void;
	onDismissTransfer?: (transferId: string) => void;
}

function PeerList(props: PeerListProps) {
	const [saving, setSaving] = createSignal(false);
	const [orderError, setOrderError] = createSignal(false);
	const [remotePeerConnected, setRemotePeerConnected] = createSignal(false);
	const peerEntries = (): PeerIdentity[] => {
		const discovered = Object.values(peers);
		const discoveredIds = discovered.map((peer) => peer.device_id);
		return [...discovered, ...getUndiscoveredTransferPeers(discoveredIds)];
	};

	const ids = () =>
		orderedPeerIds(
			peerEntries().map((peer) => peer.device_id),
			settings()?.peer_order ?? [],
		);
	async function reorder(source: string, target: string, after: boolean) {
		if (saving() || !settings()) return false;
		setSaving(true);
		setOrderError(false);
		try {
			await savePeerOrder(movePeer(settings()?.peer_order ?? [], ids(), source, target, after));
			return true;
		} catch {
			setOrderError(true);
			return false;
		} finally {
			setSaving(false);
		}
	}

	return (
		<div class="flex w-full flex-1 flex-col space-y-2">
			<RemotePeerEntry onConnectedChange={setRemotePeerConnected} />
			<Show
				when={peerEntries().length > 0 || remotePeerConnected()}
				fallback={
					<div class="flex flex-1 items-center justify-center">
						<EmptyState title={t("noDevices")} description={t("noDevicesHint")} />
					</div>
				}
			>
				<Show when={orderError()}>
					<p role="alert" class="text-xs text-destructive">
						{t("hostOrderSaveFailed")}
					</p>
				</Show>
				<HostOrder
					ids={ids()}
					disabled={saving() || !settings()}
					label={(id) => {
						const peer = peerEntries().find((peer) => peer.device_id === id);
						return `${peer?.display_name}@${formatDeviceHostname(peer?.hostname ?? "")}`;
					}}
					onMove={reorder}
				>
					{(id, header) => {
						const peer = () => peerEntries().find((peer) => peer.device_id === id);
						const availablePeer = () => peers[id];
						const expandedContent = () => {
							const currentPeer = availablePeer();
							if (!currentPeer || !props.getExpandedContent) return undefined;
							return props.getExpandedContent(currentPeer);
						};
						const addFiles = () => {
							if (!availablePeer() || !props.onAddFiles) return undefined;
							return () => {
								const currentPeer = availablePeer();
								if (currentPeer) props.onAddFiles?.(currentPeer);
							};
						};
						const addFolders = () => {
							if (!availablePeer() || !props.onAddFolders) return undefined;
							return () => {
								const currentPeer = availablePeer();
								if (currentPeer) props.onAddFolders?.(currentPeer);
							};
						};
						const addMedia = () => {
							if (!availablePeer() || !props.onAddMedia) return undefined;
							return () => {
								const currentPeer = availablePeer();
								if (currentPeer) props.onAddMedia?.(currentPeer);
							};
						};
						const onClick = () => {
							if (!availablePeer() || !props.onPeerSelect) return undefined;
							return () => {
								const currentPeer = availablePeer();
								if (currentPeer) props.onPeerSelect?.(currentPeer);
							};
						};
						return (
							<Show when={peer()}>
								{(currentPeer) => (
									<PeerCard
										peer={currentPeer()}
										headerDrag={header}
										transfers={props.getTransfers?.(id)}
										expandedContent={expandedContent()}
										dropTargetEnabled={!!availablePeer()}
										dropHighlight={!!availablePeer() && props.dropTargetId === id}
										actionsDisabled={props.actionsDisabled || !availablePeer()}
										onAddFiles={addFiles()}
										onAddFolders={addFolders()}
										onAddMedia={addMedia()}
										onClick={onClick()}
										onAbortTransfer={(id) => props.onAbortTransfer?.(id)}
										onDismissTransfer={(id) => props.onDismissTransfer?.(id)}
									/>
								)}
							</Show>
						);
					}}
				</HostOrder>
			</Show>
		</div>
	);
}

export default PeerList;
