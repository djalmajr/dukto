import { For, type JSX, Show } from "solid-js";
import EmptyState from "~/components/empty-state";
import { t } from "~/helpers/i18n";
import type { TransferSlot } from "~/routes/-components/peers/peer-card";
import PeerCard from "~/routes/-components/peers/peer-card";
import { getUndiscoveredTransferPeers } from "~/routes/-stores/transfers";
import { type PeerIdentity, type PeerInfo, peers } from "~/stores/peers";

interface PeerListProps {
	onPeerSelect?: (peer: PeerInfo) => void;
	onAddItems?: (peer: PeerInfo, directory: boolean) => void;
	getExpandedContent?: (peer: PeerInfo) => JSX.Element | undefined;
	getTransfers?: (peerId: string) => TransferSlot[] | undefined;
	actionsDisabled?: boolean;
	dropTargetId?: string;
	onAbortTransfer?: (transferId: string) => void;
	onDismissTransfer?: (transferId: string) => void;
}

function PeerList(props: PeerListProps) {
	const peerEntries = (): PeerIdentity[] => {
		const discovered = Object.values(peers);
		const discoveredIds = discovered.map((peer) => peer.device_id);
		return [...discovered, ...getUndiscoveredTransferPeers(discoveredIds)];
	};

	return (
		<div class="flex w-full flex-1 flex-col space-y-2">
			<Show
				when={peerEntries().length > 0}
				fallback={
					<div class="flex flex-1 items-center justify-center">
						<EmptyState title={t("noDevices")} description={t("noDevicesHint")} />
					</div>
				}
			>
				<For each={peerEntries()}>
					{(peer) => {
						const availablePeer = () => peers[peer.device_id];
						const expandedContent = () => {
							const currentPeer = availablePeer();
							if (!currentPeer || !props.getExpandedContent) return undefined;
							return props.getExpandedContent(currentPeer);
						};
						const addItems = () => {
							if (!availablePeer() || !props.onAddItems) return undefined;
							return (directory: boolean) => {
								const currentPeer = availablePeer();
								if (currentPeer) props.onAddItems?.(currentPeer, directory);
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
							<PeerCard
								peer={peer}
								transfers={props.getTransfers?.(peer.device_id)}
								expandedContent={expandedContent()}
								dropTargetEnabled={!!availablePeer()}
								dropHighlight={!!availablePeer() && props.dropTargetId === peer.device_id}
								actionsDisabled={props.actionsDisabled}
								onAddItems={addItems()}
								onClick={onClick()}
								onAbortTransfer={(id) => props.onAbortTransfer?.(id)}
								onDismissTransfer={(id) => props.onDismissTransfer?.(id)}
							/>
						);
					}}
				</For>
			</Show>
		</div>
	);
}

export default PeerList;
