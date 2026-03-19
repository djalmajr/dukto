import { Show } from "solid-js";
import { formatBytes } from "../../lib/format";
import { acceptIncoming, incomingRequest, rejectIncoming } from "../../stores/transfers";

function IncomingRequestDialog() {
	const req = () => incomingRequest.current;

	return (
		<Show when={req()}>
			{(r) => (
				<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
					<div class="w-full max-w-xs rounded-lg bg-white p-4 shadow-xl dark:bg-zinc-900">
						<h3 class="text-sm font-semibold">Incoming transfer</h3>
						<p class="mt-2 text-xs text-zinc-500">
							{r().item_count} {r().item_count === 1 ? "item" : "items"} &middot;{" "}
							{formatBytes(r().total_size)}
						</p>
						<p class="mt-1 text-xs text-zinc-400">From: {r().sender_device_id.slice(0, 8)}...</p>
						<div class="mt-4 flex gap-2">
							<button
								type="button"
								class="flex-1 rounded-md bg-blue-500 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-600"
								onClick={acceptIncoming}
							>
								Accept
							</button>
							<button
								type="button"
								class="flex-1 rounded-md border border-zinc-300 px-3 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
								onClick={rejectIncoming}
							>
								Reject
							</button>
						</div>
					</div>
				</div>
			)}
		</Show>
	);
}

export default IncomingRequestDialog;
