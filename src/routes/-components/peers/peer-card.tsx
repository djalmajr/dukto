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
import { nativeErrorKey } from "~/helpers/native-error";
import { formatDeviceHostname } from "~/utils/device-hostname";
import { formatBytes } from "~/utils/format";
import CarbonClose from "~icons/carbon/close";
import CarbonDocumentAdd from "~icons/carbon/document-add";
import CarbonFolderAdd from "~icons/carbon/folder-add";
import CarbonImage from "~icons/carbon/image";
import CarbonLogout from "~icons/carbon/logout";
import CarbonOverflowMenuVertical from "~icons/carbon/overflow-menu-vertical";
import CarbonStopOutline from "~icons/carbon/stop-outline";

export interface TransferSlot {
	id: string;
	direction: "send" | "receive";
	status: "waiting_approval" | "active" | "complete" | "error";
	percent: number;
	bytesSent: number;
	bytesTotal: number;
	speed?: string;
	errorMsg?: string;
	completedAt?: number;
}

export interface PeerCardProps {
	badge?: string;
	badgeTitle?: string;
	peer: {
		device_id: string;
		display_name: string;
		hostname: string;
		platform: string;
	};
	transfers?: TransferSlot[];
	expandedContent?: JSX.Element;
	headerDrag?: JSX.HTMLAttributes<HTMLDivElement>;
	dropHighlight?: boolean;
	dropTargetEnabled?: boolean;
	actionsDisabled?: boolean;
	onAddFiles?: () => void;
	onAddFolders?: () => void;
	onAddMedia?: () => void;
	onClick?: () => void;
	onDisconnect?: () => void;
	onAbortTransfer?: (transferId: string) => void;
	onDismissTransfer?: (transferId: string) => void;
}

const AUTO_DISMISS_SECS = 3;

function PeerIdentity(props: {
	badge?: string;
	badgeTitle?: string;
	peer: PeerCardProps["peer"];
}) {
	return (
		<>
			<span class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
				<PlatformIcon platform={props.peer.platform} class="h-[22px] w-[22px]" />
			</span>
			<div class="min-w-0 flex-1">
				<div class="flex min-w-0 items-center gap-2">
					<p class="truncate text-sm font-semibold tracking-[-0.15px]">{props.peer.display_name}</p>
					<Show when={props.badge}>
						{(badge) => (
							<span
								class="shrink-0 rounded-full bg-muted px-1.5 py-0.5 text-[10px] font-medium leading-none text-muted-foreground"
								title={props.badgeTitle}
							>
								{badge()}
							</span>
						)}
					</Show>
				</div>
				<p class="truncate text-xs text-muted-foreground">
					{formatDeviceHostname(props.peer.hostname)}
				</p>
			</div>
		</>
	);
}

function TransferRow(props: {
	slot: TransferSlot;
	onAbort?: () => void;
	onDismiss?: () => void;
}) {
	const isWaiting = () => props.slot.status === "waiting_approval";
	const isActive = () => props.slot.status === "active";
	const isAborting = () => isWaiting() || isActive();
	const isError = () => props.slot.status === "error";
	const isDone = () => props.slot.status === "complete";

	const label = () => {
		const dir = props.slot.direction;
		const arrow = dir === "send" ? "\u2191" : "\u2193";
		if (isWaiting()) return `${arrow} ${t("waitingForApproval")}`;
		if (isActive()) return `${arrow} ${dir === "send" ? t("sending") : t("receiving")}`;
		if (isDone()) return `${arrow} ${dir === "send" ? t("sent") : t("received")}`;
		return `${arrow} ${t("failed")}`;
	};

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
						"text-error-foreground": isError(),
					}}
				>
					{label()}
					<Show when={isActive() && props.slot.speed}> &middot; {props.slot.speed}</Show>
				</span>
				<span class="ml-auto flex shrink-0 items-center gap-1.5 tabular-nums text-muted-foreground">
					<Show
						when={isAborting()}
						fallback={
							<Button
								aria-label={t("dismissTransfer")}
								variant="ghost"
								size="icon-sm"
								class="pointer-events-auto shrink-0 text-muted-foreground [&_svg]:size-4"
								onClick={(e) => {
									e.stopPropagation();
									props.onDismiss?.();
								}}
							>
								<CarbonClose width={16} height={16} />
							</Button>
						}
					>
						<Show when={isActive()} fallback={formatBytes(props.slot.bytesTotal)}>
							{formatBytes(props.slot.bytesSent)} / {formatBytes(props.slot.bytesTotal)}
						</Show>
						<Button
							variant="ghost"
							size="icon-sm"
							class="pointer-events-auto shrink-0 text-muted-foreground [&_svg]:size-4"
							data-transfer-abort
							title={t("abortTransfer")}
							aria-label={t("abortTransfer")}
							onClick={(e) => {
								e.stopPropagation();
								props.onAbort?.();
							}}
						>
							<CarbonStopOutline width={16} height={16} />
						</Button>
					</Show>
				</span>
			</div>
			<Show when={isError() && props.slot.errorMsg}>
				<p class="text-xs text-error-foreground/90">
					{t(nativeErrorKey(props.slot.errorMsg, "transferFailed"))}
				</p>
			</Show>
			<Show when={isAborting()}>
				<div class="h-1 overflow-hidden rounded-full bg-muted">
					<Show
						when={isActive()}
						fallback={<div class="h-full w-full rounded-full bg-[#2b7fff]/40 animate-pulse" />}
					>
						<div
							class="h-full rounded-full transition-all duration-300"
							classList={{
								"bg-[#2b7fff]": props.slot.direction === "send",
								"bg-[#00c950]": props.slot.direction === "receive",
							}}
							style={{ width: `${Math.min(props.slot.percent, 100)}%` }}
						/>
					</Show>
				</div>
			</Show>
		</div>
	);
}

function PeerCard(props: PeerCardProps) {
	const slots = () => props.transfers ?? [];
	const hasActions = () =>
		Boolean(props.onAddFiles || props.onAddFolders || props.onAddMedia || props.onDisconnect);
	const actionsUnavailable = () => props.actionsDisabled || !hasActions();
	const hasActive = () =>
		slots().some((s) => s.status === "active" || s.status === "waiting_approval");
	const hasError = () => slots().some((s) => s.status === "error");
	const hasAny = () => slots().length > 0;
	const isExpanded = () => hasAny() || Boolean(props.expandedContent);

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
			<div
				{...props.headerDrag}
				class="flex w-full items-center gap-3 rounded-t-xl px-3.5 py-3.5 outline-none transition-colors data-[active=true]:bg-accent"
				classList={{
					"rounded-b-xl": !isExpanded(),
					"touch-none select-none cursor-grab active:cursor-grabbing":
						props.headerDrag?.tabIndex === 0,
				}}
			>
				<Show
					when={props.onClick}
					fallback={
						<div class="flex min-w-0 flex-1 items-center gap-3">
							<PeerIdentity badge={props.badge} badgeTitle={props.badgeTitle} peer={props.peer} />
						</div>
					}
				>
					<Button
						variant="ghost"
						class="h-auto min-w-0 flex-1 justify-start gap-3 rounded-none p-0 text-left text-foreground transition-colors hover:bg-accent/50"
						onClick={props.onClick}
					>
						<PeerIdentity badge={props.badge} badgeTitle={props.badgeTitle} peer={props.peer} />
					</Button>
				</Show>
				<Show when={hasActions() || props.actionsDisabled}>
					<DropdownMenu placement="bottom-end">
						<DropdownMenuTrigger
							as={Button}
							variant="ghost"
							size="icon-sm"
							disabled={actionsUnavailable()}
							class="peer-actions-trigger shrink-0 text-muted-foreground"
							aria-label={t("hostActions", {
								name: props.peer.display_name,
								host: formatDeviceHostname(props.peer.hostname),
							})}
						>
							<CarbonOverflowMenuVertical class="size-4" />
						</DropdownMenuTrigger>
						<DropdownMenuPortal>
							<DropdownMenuContent class="peer-actions-menu">
								<Show when={props.onAddFiles}>
									<DropdownMenuItem
										class="peer-actions-menu-item"
										disabled={actionsUnavailable()}
										onSelect={() => props.onAddFiles?.()}
									>
										<CarbonDocumentAdd class="size-4" />
										{t("addFiles")}
									</DropdownMenuItem>
								</Show>
								<Show when={props.onAddFolders}>
									<DropdownMenuItem
										class="peer-actions-menu-item"
										disabled={actionsUnavailable()}
										onSelect={() => props.onAddFolders?.()}
									>
										<CarbonFolderAdd class="size-4" />
										{t("addFolders")}
									</DropdownMenuItem>
								</Show>
								<Show when={props.onAddMedia}>
									<DropdownMenuItem
										class="peer-actions-menu-item"
										disabled={actionsUnavailable()}
										onSelect={() => props.onAddMedia?.()}
									>
										<CarbonImage class="size-4" />
										{t("addPhotosAndVideos")}
									</DropdownMenuItem>
								</Show>
								<Show when={props.onDisconnect}>
									<DropdownMenuItem
										class="peer-actions-menu-item"
										disabled={actionsUnavailable()}
										onSelect={() => props.onDisconnect?.()}
									>
										<CarbonLogout class="size-4" />
										{t("disconnectInternetSession")}
									</DropdownMenuItem>
								</Show>
							</DropdownMenuContent>
						</DropdownMenuPortal>
					</DropdownMenu>
				</Show>
			</div>
			<Show when={hasAny()}>
				<div class="border-t border-border" />
				<div class="space-y-3 p-3.5">
					<For each={slots().map((slot) => slot.id)}>
						{(id) => (
							<Show when={slots().find((slot) => slot.id === id)}>
								{(slot) => (
									<TransferRow
										slot={slot()}
										onAbort={() => props.onAbortTransfer?.(id)}
										onDismiss={() => props.onDismissTransfer?.(id)}
									/>
								)}
							</Show>
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
						{t("dropFilesToSend")}
					</div>
				</div>
			</Show>
		</div>
	);
}

export default PeerCard;
