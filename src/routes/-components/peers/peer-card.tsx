import { For, type JSX, Show, createEffect, createSignal, onCleanup } from "solid-js";
import PlatformIcon from "~/components/platform-icon";
import { Button } from "~/components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuPortal,
	DropdownMenuTrigger,
} from "~/components/ui/dropdown-menu";
import { t } from "~/helpers/i18n";
import { formatBytes } from "~/utils/format";
import LucideBan from "~icons/lucide/ban";
import LucideEllipsisVertical from "~icons/lucide/ellipsis-vertical";
import LucideFilePlus from "~icons/lucide/file-plus";
import LucideFolderPlus from "~icons/lucide/folder-plus";
import LucideX from "~icons/lucide/x";

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
	dropHighlight?: boolean;
	dropTargetEnabled?: boolean;
	actionsDisabled?: boolean;
	onAddItems?: (directory: boolean) => void;
	onClick?: () => void;
	onAbortTransfer?: (transferId: string) => void;
	onDismissTransfer?: (transferId: string) => void;
}

const AUTO_DISMISS_SECS = 3;

function PeerIdentity(props: { peer: PeerCardProps["peer"] }) {
	return (
		<>
			<span class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
				<PlatformIcon platform={props.peer.platform} class="h-[22px] w-[22px]" />
			</span>
			<div class="min-w-0 flex-1">
				<p class="truncate text-sm font-semibold tracking-[-0.15px]">{props.peer.display_name}</p>
				<p class="truncate text-xs text-muted-foreground">{props.peer.hostname}</p>
			</div>
		</>
	);
}

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

	return (
		<div
			data-peer-id={props.dropTargetEnabled === false ? undefined : props.peer.device_id}
			class="relative shrink-0 overflow-hidden rounded-xl border bg-card shadow-[0_1px_6px_0_rgba(0,0,0,0.05)] transition-colors"
			classList={{
				"border-blue-500 ring-2 ring-blue-500 ring-inset bg-blue-50/50 dark:bg-blue-950/20":
					!!props.dropHighlight,
				"border-primary/30": !props.dropHighlight && hasActive() && !hasError(),
				"border-destructive/30": !props.dropHighlight && hasError(),
				"border-border": !props.dropHighlight && !hasActive() && !hasError(),
			}}
		>
			<div class="flex w-full items-center gap-3 px-3.5 py-3.5">
				<Show
					when={props.onClick}
					fallback={
						<div class="flex min-w-0 flex-1 items-center gap-3">
							<PeerIdentity peer={props.peer} />
						</div>
					}
				>
					<Button
						variant="ghost"
						class="h-auto min-w-0 flex-1 justify-start gap-3 rounded-none p-0 text-left text-foreground transition-colors hover:bg-accent/50"
						onClick={props.onClick}
					>
						<PeerIdentity peer={props.peer} />
					</Button>
				</Show>
				<Show when={props.onAddItems}>
					<DropdownMenu placement="bottom-end">
						<DropdownMenuTrigger
							as={Button}
							variant="ghost"
							size="icon"
							disabled={props.actionsDisabled}
							class="size-8 shrink-0 text-muted-foreground"
							aria-label={`Actions for ${props.peer.display_name} (${props.peer.hostname})`}
						>
							<LucideEllipsisVertical class="size-4" />
						</DropdownMenuTrigger>
						<DropdownMenuPortal>
							<DropdownMenuContent>
								<DropdownMenuItem
									disabled={props.actionsDisabled}
									onSelect={() => props.onAddItems?.(false)}
								>
									<LucideFilePlus class="size-4" />
									{t("addFiles")}
								</DropdownMenuItem>
								<DropdownMenuItem
									disabled={props.actionsDisabled}
									onSelect={() => props.onAddItems?.(true)}
								>
									<LucideFolderPlus class="size-4" />
									{t("addFolders")}
								</DropdownMenuItem>
							</DropdownMenuContent>
						</DropdownMenuPortal>
					</DropdownMenu>
				</Show>
			</div>
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
			<Show when={props.dropHighlight}>
				<div class="pointer-events-none absolute inset-0 flex items-center justify-center bg-blue-50/60 dark:bg-blue-950/40">
					<div class="rounded-lg bg-blue-500 px-4 py-2 text-xs font-medium text-white shadow-lg">
						Drop files to send
					</div>
				</div>
			</Show>
		</div>
	);
}

export default PeerCard;
