import { For, type JSX, Show, createEffect, createSignal, onCleanup } from "solid-js";
import Icon from "~/components/icon";
import { formatBytes } from "~/lib/format";
import { platformIcon } from "~/lib/platform";

export interface TransferSlot {
	status: "active" | "complete" | "error";
	percent: number;
	bytesSent: number;
	bytesTotal: number;
	speed?: string;
	errorMsg?: string;
	completedAt?: number;
}

export interface PeerTransfers {
	send?: TransferSlot;
	receive?: TransferSlot;
}

export interface PeerCardProps {
	peer: {
		device_id: string;
		display_name: string;
		hostname: string;
		platform: string;
	};
	transfers?: PeerTransfers;
	expandedContent?: JSX.Element;
	onClick?: () => void;
	onDismissTransfer?: (direction: "send" | "receive") => void;
}

const AUTO_DISMISS_SECS = 10;

function TransferRow(props: {
	direction: "send" | "receive";
	slot: TransferSlot;
	onDismiss?: () => void;
}) {
	const label = () => {
		if (props.slot.status === "active")
			return props.direction === "send" ? "\u2191 Sending..." : "\u2193 Receiving...";
		if (props.slot.status === "complete")
			return props.direction === "send" ? "\u2191 Sent" : "\u2193 Received";
		return props.direction === "send" ? "\u2191 Failed" : "\u2193 Failed";
	};

	const isError = () => props.slot.status === "error";
	const isDone = () => props.slot.status === "complete";

	let countdownTimer: number | undefined;

	createEffect(() => {
		clearInterval(countdownTimer);
		const completedAt = props.slot.completedAt;
		if (isDone() && completedAt) {
			const elapsed = Math.floor((Date.now() - completedAt) / 1000);
			const left = Math.max(AUTO_DISMISS_SECS - elapsed, 0);
			if (left <= 0) {
				props.onDismiss?.();
				return;
			}
			countdownTimer = window.setInterval(() => {
				const next = Math.max(AUTO_DISMISS_SECS - Math.floor((Date.now() - completedAt) / 1000), 0);
				if (next <= 0) {
					clearInterval(countdownTimer);
					props.onDismiss?.();
				}
			}, 1000);
		}
	});
	onCleanup(() => clearInterval(countdownTimer));

	return (
		<div class="space-y-1">
			<div
				class="flex items-center gap-2 text-xs"
				classList={{
					"text-muted-foreground": !isError(),
					"text-destructive": isError(),
				}}
			>
				<span class="min-w-0 truncate">
					{label()}
					<Show when={props.slot.status === "active" && props.slot.speed}>
						{" "}
						&middot; {props.slot.speed}
					</Show>
				</span>
				<span class="ml-auto flex shrink-0 items-center gap-2 tabular-nums">
					<Show
						when={props.slot.status === "active"}
						fallback={
							<button
								type="button"
								class="pointer-events-auto flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
								onClick={(e) => {
									e.stopPropagation();
									props.onDismiss?.();
								}}
							>
								<svg
									class="h-3 w-3"
									fill="none"
									stroke="currentColor"
									stroke-width="2"
									viewBox="0 0 24 24"
									aria-hidden="true"
								>
									<path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
								</svg>
							</button>
						}
					>
						{formatBytes(props.slot.bytesSent)} / {formatBytes(props.slot.bytesTotal)}
					</Show>
				</span>
			</div>
			<Show when={isError() && props.slot.errorMsg}>
				<p class="text-xs text-destructive/80">{props.slot.errorMsg}</p>
			</Show>
			<Show when={props.slot.status === "active"}>
				<div class="h-1 overflow-hidden rounded-full bg-secondary">
					<div
						class="h-full rounded-full transition-all duration-300"
						classList={{
							"bg-blue-500": props.direction === "send",
							"bg-green-500": props.direction === "receive",
						}}
						style={{ width: `${Math.min(props.slot.percent, 100)}%` }}
					/>
				</div>
			</Show>
		</div>
	);
}

function PeerCard(props: PeerCardProps) {
	const iconName = () => platformIcon(props.peer.platform);
	const hasActive = () =>
		props.transfers?.send?.status === "active" || props.transfers?.receive?.status === "active";
	const hasError = () =>
		props.transfers?.send?.status === "error" || props.transfers?.receive?.status === "error";
	const hasAny = () => !!(props.transfers?.send || props.transfers?.receive);
	const hasExpanded = () => !!props.expandedContent;

	const slots = () => {
		const result: { direction: "send" | "receive"; slot: TransferSlot }[] = [];
		if (props.transfers?.send) result.push({ direction: "send", slot: props.transfers.send });
		if (props.transfers?.receive)
			result.push({ direction: "receive", slot: props.transfers.receive });
		return result;
	};

	return (
		<div
			class="overflow-hidden rounded-lg border border-border bg-card shadow-sm transition-colors"
			classList={{
				"border-primary/30": hasActive() && !hasError(),
				"border-destructive/30": hasError(),
				"border-primary/40": hasExpanded() && !hasAny() && !hasError(),
			}}
		>
			<button
				type="button"
				class="flex w-full items-center gap-3 p-3.5 text-left transition-colors"
				classList={{
					"hover:bg-accent/50": !hasAny() && !hasExpanded(),
					"pointer-events-none": hasAny() || hasExpanded(),
				}}
				onClick={props.onClick}
			>
				<span class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
					<Icon name={iconName()} size={22} />
				</span>
				<div class="min-w-0 flex-1">
					<p class="truncate text-sm font-semibold">{props.peer.display_name}</p>
					<p class="truncate text-xs text-muted-foreground">{props.peer.hostname}</p>
					<Show when={hasAny()}>
						<div class="mt-1.5 space-y-1.5">
							<For each={slots()}>
								{(item) => (
									<TransferRow
										direction={item.direction}
										slot={item.slot}
										onDismiss={() => props.onDismissTransfer?.(item.direction)}
									/>
								)}
							</For>
						</div>
					</Show>
				</div>
			</button>
			<Show when={props.expandedContent}>
				<div class="border-t border-border bg-background/80 p-3">{props.expandedContent}</div>
			</Show>
		</div>
	);
}

export default PeerCard;
