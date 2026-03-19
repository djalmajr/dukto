import { Show } from "solid-js";
import { formatBytes } from "~/lib/format";

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
		<div class="rounded-lg border border-border bg-card p-3 shadow-sm">
			<div class="flex items-center justify-between text-xs">
				<span class="font-medium">
					{arrow()} {label()}
				</span>
				<span class="tabular-nums text-muted-foreground">
					{formatBytes(props.bytesSent)} / {formatBytes(props.bytesTotal)}
				</span>
			</div>
			<div class="mt-2 h-1.5 overflow-hidden rounded-full bg-secondary">
				<div
					class="h-full rounded-full transition-all duration-300"
					classList={{
						"bg-primary": props.status === "active",
						"bg-success-foreground": props.status === "complete",
						"bg-destructive": props.status === "error" || props.status === "rejected",
					}}
					style={{ width: `${Math.min(props.percent, 100)}%` }}
				/>
			</div>
			<Show when={props.peerName || props.speed}>
				<p class="mt-1.5 text-[11px] text-muted-foreground">
					{props.direction === "send" ? "To" : "From"}: {props.peerName}
					{props.speed && ` \u00b7 ${props.speed}`}
				</p>
			</Show>
			<Show when={props.errorMsg}>
				<p class="mt-1 text-[11px] text-destructive">{props.errorMsg}</p>
			</Show>
			<Show when={isDone() && props.onDismiss}>
				<button
					type="button"
					class="mt-2 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
					onClick={props.onDismiss}
				>
					Dismiss
				</button>
			</Show>
		</div>
	);
}

export default TransferBar;
