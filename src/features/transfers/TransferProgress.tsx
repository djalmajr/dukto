import { For, Show } from "solid-js";
import { formatBytes } from "../../lib/format";
import { type ActiveTransfer, clearTransfer, transfers } from "../../stores/transfers";

function TransferList() {
	const activeTransfers = () => Object.values(transfers);

	return (
		<Show when={activeTransfers().length > 0}>
			<div class="space-y-2">
				<h2 class="text-sm font-semibold text-zinc-500">Transfers</h2>
				<For each={activeTransfers()}>{(t) => <TransferItem transfer={t} />}</For>
			</div>
		</Show>
	);
}

function TransferItem(props: { transfer: ActiveTransfer }) {
	const statusLabel = () => {
		switch (props.transfer.status) {
			case "sending":
				return "Sending...";
			case "receiving":
				return "Receiving...";
			case "complete":
				return "Complete";
			case "error":
				return "Failed";
			case "rejected":
				return "Rejected";
			default:
				return "";
		}
	};

	const isDone = () =>
		props.transfer.status === "complete" ||
		props.transfer.status === "error" ||
		props.transfer.status === "rejected";

	return (
		<div class="rounded-lg border border-zinc-200 p-3 dark:border-zinc-800">
			<div class="flex items-center justify-between text-xs">
				<span class="font-medium">
					{props.transfer.direction === "send" ? "↑" : "↓"} {statusLabel()}
				</span>
				<span class="text-zinc-400">
					{formatBytes(props.transfer.bytes_sent)} / {formatBytes(props.transfer.bytes_total)}
				</span>
			</div>
			<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-700">
				<div
					class="h-full rounded-full transition-all duration-200"
					classList={{
						"bg-blue-500": !isDone(),
						"bg-green-500": props.transfer.status === "complete",
						"bg-red-500": props.transfer.status === "error" || props.transfer.status === "rejected",
					}}
					style={{ width: `${Math.min(props.transfer.percent, 100)}%` }}
				/>
			</div>
			<Show when={props.transfer.error}>
				<p class="mt-1 text-xs text-red-500">{props.transfer.error}</p>
			</Show>
			<Show when={isDone()}>
				<button
					type="button"
					class="mt-2 text-xs text-zinc-400 hover:text-zinc-600"
					onClick={() => clearTransfer(props.transfer.transfer_id)}
				>
					Dismiss
				</button>
			</Show>
		</div>
	);
}

export default TransferList;
