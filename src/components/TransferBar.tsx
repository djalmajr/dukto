import { Show } from "solid-js";
import { formatBytes } from "../lib/format";

interface TransferBarProps {
	direction: "send" | "receive";
	status: "active" | "complete" | "error" | "rejected";
	percent: number;
	bytesSent: number;
	bytesTotal: number;
	peerName?: string;
	speed?: string;
	errorMsg?: string;
	onDismiss?: () => void;
}

function TransferBar(props: TransferBarProps) {
	const arrow = () => (props.direction === "send" ? "\u2191" : "\u2193");
	const label = () => {
		switch (props.status) {
			case "active":
				return props.direction === "send" ? "Sending..." : "Receiving...";
			case "complete":
				return "Complete";
			case "error":
				return "Failed";
			case "rejected":
				return "Rejected";
		}
	};
	const isDone = () => props.status !== "active";

	return (
		<div class="rounded-lg border border-zinc-200 p-3 transition-colors dark:border-zinc-800">
			<div class="flex items-center justify-between text-xs">
				<span class="font-medium">
					{arrow()} {label()}
				</span>
				<span class="text-zinc-400">
					{formatBytes(props.bytesSent)} / {formatBytes(props.bytesTotal)}
				</span>
			</div>
			<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-zinc-200 dark:bg-zinc-700">
				<div
					class="h-full rounded-full transition-all duration-300"
					classList={{
						"bg-blue-500": props.status === "active",
						"bg-green-500": props.status === "complete",
						"bg-red-500": props.status === "error" || props.status === "rejected",
					}}
					style={{ width: `${Math.min(props.percent, 100)}%` }}
				/>
			</div>
			<Show when={props.peerName || props.speed}>
				<p class="mt-1 text-[11px] text-zinc-500">
					{props.direction === "send" ? "To" : "From"}: {props.peerName}
					{props.speed && ` \u00b7 ${props.speed}`}
				</p>
			</Show>
			<Show when={props.errorMsg}>
				<p class="mt-1 text-[11px] text-red-500">{props.errorMsg}</p>
			</Show>
			<Show when={isDone() && props.onDismiss}>
				<button
					type="button"
					class="mt-2 text-[11px] text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
					onClick={props.onDismiss}
				>
					Dismiss
				</button>
			</Show>
		</div>
	);
}

export default TransferBar;
