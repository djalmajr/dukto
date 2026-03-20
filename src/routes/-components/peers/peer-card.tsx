import { For, type JSX, Show, createEffect, createSignal, onCleanup } from "solid-js";
import LucideBan from "~icons/lucide/ban";
import LucideX from "~icons/lucide/x";
import PlatformIcon from "~/components/platform-icon";
import { t } from "~/helpers/i18n";
import { formatBytes } from "~/utils/format";

export interface TransferSlot {
	id: string;
	direction: "send" | "receive";
	status: "active" | "complete" | "error";
	percent: number;
	bytesSent: number;
	bytesTotal: number;
	speed?: string;
	errorMsg?: string;
	completedAt?: number;
}

export interface PeerCardProps {
	peer: {
		device_id: string;
		display_name: string;
		hostname: string;
		platform: string;
	};
	transfers?: TransferSlot[];
	expandedContent?: JSX.Element;
	onClick?: () => void;
	onAbortTransfer?: (transferId: string) => void;
	onDismissTransfer?: (transferId: string) => void;
}

const AUTO_DISMISS_SECS = 3;

function TransferRow(props: {
	slot: TransferSlot;
	onAbort?: () => void;
	onDismiss?: () => void;
}) {
	const label = () => {
		const dir = props.slot.direction;
		const arrow = dir === "send" ? "\u2191" : "\u2193";
		if (props.slot.status === "active")
			return `${arrow} ${dir === "send" ? t("sending") : t("receiving")}`;
		if (props.slot.status === "complete")
			return `${arrow} ${dir === "send" ? t("sent") : t("received")}`;
		return `${arrow} ${t("failed")}`;
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
			<div class="flex items-center gap-2 text-xs">
				<span
					class="min-w-0 truncate"
					classList={{
						"text-muted-foreground": !isError(),
						"text-destructive": isError(),
					}}
				>
					{label()}
					<Show when={props.slot.status === "active" && props.slot.speed}>
						{" "}
						&middot; {props.slot.speed}
					</Show>
				</span>
				<span class="ml-auto flex shrink-0 items-center gap-1.5 tabular-nums text-muted-foreground">
					<Show when={props.slot.status === "active"}>
						{formatBytes(props.slot.bytesSent)} / {formatBytes(props.slot.bytesTotal)}
						<button
							type="button"
							class="pointer-events-auto flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
							title="Abort transfer"
							onClick={(e) => {
								e.stopPropagation();
								props.onAbort?.();
							}}
						>
							<LucideBan width={12} height={12} />
						</button>
					</Show>
					<Show when={props.slot.status !== "active"}>
						<button
							type="button"
							class="pointer-events-auto flex h-5 w-5 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
							onClick={(e) => {
								e.stopPropagation();
								props.onDismiss?.();
							}}
						>
							<LucideX width={12} height={12} />
						</button>
					</Show>
				</span>
			</div>
			<Show when={isError() && props.slot.errorMsg}>
				<p class="text-xs text-destructive/80">{props.slot.errorMsg}</p>
			</Show>
			<Show when={props.slot.status === "active"}>
				<div class="h-1 overflow-hidden rounded-full bg-muted">
					<div
						class="h-full rounded-full transition-all duration-300"
						classList={{
							"bg-[#2b7fff]": props.slot.direction === "send",
							"bg-[#00c950]": props.slot.direction === "receive",
						}}
						style={{ width: `${Math.min(props.slot.percent, 100)}%` }}
					/>
				</div>
			</Show>
		</div>
	);
}

function PeerCard(props: PeerCardProps) {
	const slots = () => props.transfers ?? [];
	const hasActive = () => slots().some((s) => s.status === "active");
	const hasError = () => slots().some((s) => s.status === "error");
	const hasAny = () => slots().length > 0;
	const hasExpanded = () => !!props.expandedContent;

	return (
		<div
			class="overflow-hidden rounded-xl border border-border bg-card shadow-[0_1px_6px_0_rgba(0,0,0,0.05)] transition-colors"
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
					"hover:bg-accent/50": !hasExpanded(),
					"pointer-events-none": hasExpanded(),
				}}
				onClick={props.onClick}
			>
				<span class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
					<PlatformIcon platform={props.peer.platform} class="h-[22px] w-[22px]" />
				</span>
				<div class="min-w-0 flex-1">
					<p class="truncate text-sm font-semibold">{props.peer.display_name}</p>
					<p class="truncate text-xs text-muted-foreground">{props.peer.hostname}</p>
				</div>
			</button>
			<Show when={hasAny()}>
				<div class="border-t border-border" />
				<div class="space-y-3 p-3.5">
					<For each={slots()}>
						{(slot) => (
							<TransferRow
								slot={slot}
								onAbort={() => props.onAbortTransfer?.(slot.id)}
								onDismiss={() => props.onDismissTransfer?.(slot.id)}
							/>
						)}
					</For>
				</div>
			</Show>
			<Show when={props.expandedContent}>
				<div class="border-t border-border bg-background/80 p-3">{props.expandedContent}</div>
			</Show>
		</div>
	);
}

export default PeerCard;
