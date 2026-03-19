import { Show } from "solid-js";
import { formatBytes } from "../lib/format";
import { resolvedTheme } from "../stores/app";

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
	const dark = () => resolvedTheme() === "dark";
	const arrow = () => (props.direction === "send" ? "↑" : "↓");
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

	return (
		<div
			class="rounded-lg border p-3 transition-colors"
			classList={{ "border-zinc-200": !dark(), "border-zinc-800": dark() }}
		>
			<div class="flex items-center justify-between text-xs">
				<span class="font-medium">
					{arrow()} {label()}
				</span>
				<span class="text-zinc-400">
					{formatBytes(props.bytesSent)} / {formatBytes(props.bytesTotal)}
				</span>
			</div>
			<div
				class="mt-2 h-1.5 overflow-hidden rounded-full transition-colors"
				classList={{ "bg-zinc-200": !dark(), "bg-zinc-700": dark() }}
			>
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
					{props.speed && ` · ${props.speed}`}
				</p>
			</Show>
			<Show when={props.errorMsg}>
				<p class="mt-1 text-[11px] text-red-500">{props.errorMsg}</p>
			</Show>
			<Show when={props.status !== "active" && props.onDismiss}>
				<button
					type="button"
					class="mt-2 text-[11px] text-zinc-400 hover:text-zinc-300"
					onClick={props.onDismiss}
				>
					Dismiss
				</button>
			</Show>
		</div>
	);
}

export default TransferBar;
