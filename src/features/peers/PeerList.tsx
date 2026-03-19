import { For, Show } from "solid-js";
import EmptyState from "../../components/EmptyState";
import { type PeerInfo, peers } from "../../stores/peers";
import PeerCard from "./PeerCard";

interface PeerListProps {
	onPeerSelect?: (peer: PeerInfo) => void;
}

function PeerList(props: PeerListProps) {
	const peerEntries = () => Object.values(peers);

	return (
		<div class="w-full space-y-2">
			<Show
				when={peerEntries().length > 0}
				fallback={
					<EmptyState
						title="No devices found"
						description="Make sure other devices are running Dukto on the same network"
					/>
				}
			>
				<For each={peerEntries()}>
					{(peer) => <PeerCard peer={peer} onSelect={props.onPeerSelect} />}
				</For>
			</Show>
		</div>
	);
}

export default PeerList;
