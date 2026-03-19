import { For, Show } from "solid-js";
import { type PeerInfo, peers } from "~/stores/peers";

interface PeerSelectorProps {
	onSelect: (peer: PeerInfo) => void;
	onCancel: () => void;
}

function PeerSelector(props: PeerSelectorProps) {
	const peerList = () => Object.values(peers);

	return (
		<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
			<div class="w-full max-w-xs rounded-lg bg-white p-4 shadow-xl dark:bg-zinc-900">
				<h3 class="text-sm font-semibold">Select recipient</h3>
				<Show
					when={peerList().length > 0}
					fallback={<p class="py-4 text-center text-xs text-zinc-400">No devices available</p>}
				>
					<ul class="mt-3 space-y-1">
						<For each={peerList()}>
							{(peer) => (
								<li>
									<button
										type="button"
										class="w-full rounded-md px-3 py-2 text-left text-sm hover:bg-zinc-100 dark:hover:bg-zinc-800"
										onClick={() => props.onSelect(peer)}
									>
										<span class="font-medium">{peer.display_name}</span>
										<span class="ml-2 text-xs text-zinc-400">{peer.hostname}</span>
									</button>
								</li>
							)}
						</For>
					</ul>
				</Show>
				<button
					type="button"
					class="mt-3 w-full rounded-md border border-zinc-300 px-3 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
					onClick={props.onCancel}
				>
					Cancel
				</button>
			</div>
		</div>
	);
}

export default PeerSelector;
