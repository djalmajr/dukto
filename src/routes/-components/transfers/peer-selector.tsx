import { For, Show } from "solid-js";
import { Button } from "~/components/ui/button";
import { t } from "~/helpers/i18n";
import { type PeerInfo, peers } from "~/stores/peers";

interface PeerSelectorProps {
	onSelect: (peer: PeerInfo) => void;
	onCancel: () => void;
}

function PeerSelector(props: PeerSelectorProps) {
	const peerList = () => Object.values(peers);

	return (
		<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
			<div class="w-full max-w-xs rounded-lg bg-card p-4 shadow-xl">
				<h3 class="text-sm font-semibold">Select recipient</h3>
				<Show
					when={peerList().length > 0}
					fallback={<p class="py-4 text-center text-xs text-muted-foreground">{t("noDevices")}</p>}
				>
					<ul class="mt-3 space-y-1">
						<For each={peerList()}>
							{(peer) => (
								<li>
									<button
										type="button"
										class="w-full rounded-md px-3 py-2 text-left text-sm hover:bg-accent"
										onClick={() => props.onSelect(peer)}
									>
										<span class="font-medium">{peer.display_name}</span>
										<span class="ml-2 text-xs text-muted-foreground">{peer.hostname}</span>
									</button>
								</li>
							)}
						</For>
					</ul>
				</Show>
				<Button variant="outline" class="mt-3 w-full" onClick={props.onCancel}>
					{t("cancel")}
				</Button>
			</div>
		</div>
	);
}

export default PeerSelector;
