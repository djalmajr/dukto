import { For, type JSX, Show } from "solid-js";
import EmptyState from "~/components/empty-state";
import { t } from "~/lib/i18n";
import type { TransferSlot } from "~/features/peers/peer-card";
import PeerCard from "~/features/peers/peer-card";
import { type PeerInfo, peers } from "~/stores/peers";

interface PeerListProps {
	onPeerSelect?: (peer: PeerInfo) => void;
	getExpandedContent?: (peer: PeerInfo) => JSX.Element | undefined;
	getTransfers?: (peer: PeerInfo) => TransferSlot[] | undefined;
	onAbortTransfer?: (transferId: string) => void;
	onDismissTransfer?: (transferId: string) => void;
}

function PeerList(props: PeerListProps) {
	const peerEntries = () => Object.values(peers);

	return (
		<div class="w-full space-y-2">
			<Show
				when={peerEntries().length > 0}
				fallback={
					<EmptyState
						title={t("noDevices")}
						description={t("noDevicesHint")}
					/>
				}
			>
				<For each={peerEntries()}>
					{(peer) => (
						<PeerCard
							peer={peer}
							transfers={props.getTransfers?.(peer)}
							expandedContent={props.getExpandedContent?.(peer)}
							onClick={() => props.onPeerSelect?.(peer)}
							onAbortTransfer={(id) => props.onAbortTransfer?.(id)}
							onDismissTransfer={(id) => props.onDismissTransfer?.(id)}
						/>
					)}
				</For>
			</Show>
		</div>
	);
}

export default PeerList;
