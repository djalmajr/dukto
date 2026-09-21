import { useNavigate, useSearch } from "@tanstack/solid-router";
import { listen } from "@tauri-apps/api/event";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { platform } from "@tauri-apps/plugin-os";
import { Show, createEffect, createSignal, onCleanup, onMount } from "solid-js";
import { Button } from "~/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
} from "~/components/ui/dialog";
import { TextField, TextFieldInput, TextFieldLabel } from "~/components/ui/text-field";
import Toast from "~/components/ui/toast";
import { t } from "~/helpers/i18n";
import { nativeErrorKey } from "~/helpers/native-error";
import {
	pickSendItems,
	supportsFolderSelection,
	supportsMediaSelection,
} from "~/helpers/send-item-picker";
import {
	type FileMetadataInfo,
	type InternetInviteShareView,
	type InternetInviteView,
	cancelInternetInvite,
	confirmInternetPairingCode,
	createInternetInvite,
	disconnectInternetSession,
	getInternetInvitationLink,
	importInternetInvite,
	releaseSelectedFiles,
	sendToInternetSession,
} from "~/helpers/tauri";
import SendPreview from "~/routes/-components/transfers/send-preview";
import { startSendTransfer } from "~/routes/-stores/transfers";
import type { PeerIdentity } from "~/stores/peers";
import CarbonCircleDash from "~icons/carbon/circle-dash";
import CarbonClose from "~icons/carbon/close";
import CarbonCopy from "~icons/carbon/copy";
import CarbonDirectLink from "~icons/carbon/direct-link";
import CarbonLink from "~icons/carbon/link";
import CarbonShare from "~icons/carbon/share";
import PeerCard, { type TransferSlot } from "./peer-card";
import RemoteSessionStatus from "./remote-session-status";

type PendingAction =
	| "cancel"
	| "connect"
	| "copy"
	| "create"
	| "disconnect"
	| "pick"
	| "send"
	| "share"
	| null;

type InvitationToast = "invitationLinkCopied" | "invitationQrCopied" | "invitationQrShared";

interface RemotePeerEntryProps {
	onConnectedChange?: (connected: boolean) => void;
	onPeerIdentityChange?: (peer: PeerIdentity | null) => void;
	getTransfers?: (peerId: string) => TransferSlot[] | undefined;
	onAbortTransfer?: (transferId: string) => void;
	onDismissTransfer?: (transferId: string) => void;
}

function RemotePeerEntry(props: RemotePeerEntryProps) {
	const navigate = useNavigate();
	const search = useSearch({ strict: false });
	const currentPlatform = (() => {
		try {
			return platform();
		} catch {
			return "unknown";
		}
	})();
	const [invitation, setInvitation] = createSignal("");
	const [session, setSession] = createSignal<InternetInviteView | null>(null);
	const [share, setShare] = createSignal<InternetInviteShareView | null>(null);
	const [pairingCode, setPairingCode] = createSignal("");
	const [invitationToast, setInvitationToast] = createSignal<InvitationToast | null>(null);
	const [pendingAction, setPendingAction] = createSignal<PendingAction>(null);
	const [selectedFiles, setSelectedFiles] = createSignal<FileMetadataInfo[]>([]);
	const [error, setError] = createSignal<string | null>(null);
	const pending = () => pendingAction() !== null;
	const connecting = () => pendingAction() === "connect";
	let invitationToastTimer: ReturnType<typeof setTimeout> | undefined;
	function showInvitationToast(message: InvitationToast) {
		setInvitationToast(message);
		clearTimeout(invitationToastTimer);
		invitationToastTimer = setTimeout(() => setInvitationToast(null), 3500);
	}
	const isConnected = () =>
		session()?.status.state === "ready" || session()?.status.state === "transferring";
	const remotePeer = (): PeerIdentity | null => {
		const current = session();
		if (!current || !isConnected()) return null;
		return (
			current.peer ?? {
				device_id: current.peer_id,
				display_name: current.peer_id.slice(0, 8),
				hostname: "internet",
				platform: "unknown",
			}
		);
	};
	const visibleRemotePeer = (): PeerIdentity | null =>
		remotePeer() ??
		(connecting()
			? {
					device_id: "internet-connecting",
					display_name: t("internetPeerConnecting"),
					hostname: t("internetPeerHostname"),
					platform: "internet",
				}
			: null);
	const showInternetDialog = () => (search() as { internet?: boolean }).internet === true;
	const localizedError = () => {
		const message = error();
		return message ? t(nativeErrorKey(message)) : null;
	};
	const routeLabel = () => {
		const route = session()?.status.route;
		if (!isConnected() || !route) return undefined;
		return route === "direct" ? t("internetRouteDirect") : t("internetRouteRelay");
	};
	const routeTitle = () => {
		const route = session()?.status.route;
		if (!isConnected() || !route) return undefined;
		return route === "direct" ? t("remoteStatusReadyDirect") : t("remoteStatusReadyRelay");
	};
	const closeInternetDialog = () =>
		navigate({ to: "/", search: { internet: undefined, settings: undefined } });
	function applySession(next: InternetInviteView) {
		setSession(next);
		if (next.status.state === "ready" || next.status.state === "transferring") {
			closeInternetDialog();
		}
	}

	createEffect(() => props.onConnectedChange?.(isConnected() || connecting()));
	createEffect(() => props.onPeerIdentityChange?.(remotePeer()));

	async function consumeDeepLinks(urls: string[] | null) {
		for (const url of urls ?? []) {
			if (!url.startsWith("dukto://connect#")) continue;
			if (session()) {
				setError("Disconnect the current internet session before connecting another device.");
				continue;
			}
			try {
				setPendingAction("connect");
				clearSelectedFiles();
				await navigate({ to: "/", search: { internet: true, settings: undefined } });
				applySession(await importInternetInvite(url));
				setShare(null);
				setInvitationToast(null);
				setInvitation("");
				setPairingCode("");
				setError(null);
			} catch (reason) {
				setError(String(reason));
			} finally {
				setPendingAction(null);
			}
		}
	}

	async function confirmPairingCode() {
		const current = session();
		const code = pairingCode().trim();
		if (pending() || !current || !/^\d{8}$/.test(code)) return;
		setPendingAction("connect");
		setError(null);
		try {
			applySession(await confirmInternetPairingCode(current.session_id, code));
			setPairingCode("");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	onMount(() => {
		let active = true;
		let unlisten: (() => void) | undefined;
		let unlistenSession: (() => void) | undefined;
		void getCurrent().then((urls) => active && void consumeDeepLinks(urls));
		void onOpenUrl((urls) => {
			if (active) void consumeDeepLinks(urls);
		}).then((stop) => {
			if (active) unlisten = stop;
			else stop();
		});
		void listen<InternetInviteView>("internet:session-updated", ({ payload }) => {
			if (!active || session()?.session_id !== payload.session_id) return;
			const inviteExpiredBeforeConnection = payload.status.state === "expired" && !isConnected();
			if (
				payload.status.state === "failed" ||
				payload.status.state === "cancelled" ||
				inviteExpiredBeforeConnection
			) {
				clearSelectedFiles();
				setSession(null);
				setShare(null);
				setInvitationToast(null);
				setInvitation("");
				if (payload.status.state === "failed") setError("connection lost");
				return;
			}
			applySession(payload);
			if (payload.status.state !== "invited" && payload.status.state !== "pairing") {
				setShare(null);
				setInvitationToast(null);
			}
		}).then((stop) => {
			if (active) unlistenSession = stop;
			else stop();
		});
		onCleanup(() => {
			active = false;
			unlisten?.();
			unlistenSession?.();
			releaseFiles(selectedFiles());
			clearTimeout(invitationToastTimer);
		});
	});

	function releaseFiles(files: FileMetadataInfo[]) {
		void releaseSelectedFiles(files.map((file) => file.path)).catch(() => undefined);
	}

	async function pickItems(directory: boolean, media = false) {
		const current = session();
		if (pending() || current?.status.state !== "ready" || !current.can_send || share()) return;
		setPendingAction("pick");
		setError(null);
		try {
			const picked = await pickSendItems(currentPlatform, directory, media);
			if (!picked) return;
			const currentPaths = new Set(selectedFiles().map((file) => file.path));
			const fresh = picked.filter((file) => !currentPaths.has(file.path));
			releaseFiles(picked.filter((file) => currentPaths.has(file.path)));
			setSelectedFiles((files) => [...files, ...fresh]);
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	function clearSelectedFiles() {
		const files = selectedFiles();
		setSelectedFiles([]);
		releaseFiles(files);
	}

	function removeSelectedFile(path: string) {
		const removed = selectedFiles().filter((file) => file.path === path);
		setSelectedFiles((files) => files.filter((file) => file.path !== path));
		releaseFiles(removed);
	}

	async function sendSelectedFiles() {
		const current = session();
		const files = selectedFiles();
		if (pending() || current?.status.state !== "ready" || !current.can_send || files.length === 0)
			return;
		setPendingAction("send");
		setError(null);
		try {
			const transferId = await sendToInternetSession(
				current.session_id,
				files.map((file) => file.path),
			);
			const totalSize = files.reduce((sum, file) => sum + file.size, 0);
			const peer = current.peer ?? remotePeer();
			startSendTransfer(
				transferId,
				totalSize,
				peer?.device_id ?? current.peer_id,
				peer ?? undefined,
			);
			setSelectedFiles([]);
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	async function createInvitation() {
		if (pending() || session()) return;
		setPendingAction("create");
		setError(null);
		try {
			clearSelectedFiles();
			const created = await createInternetInvite();
			applySession(created);
			setShare(created);
			setInvitation(await getInternetInvitationLink(created.session_id));
			setInvitationToast(null);
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	async function connect() {
		const value = invitation().trim();
		if (pending() || session() || !value) return;
		setPendingAction("connect");
		setError(null);
		try {
			clearSelectedFiles();
			applySession(await importInternetInvite(value));
			setShare(null);
			setInvitationToast(null);
			setInvitation("");
			setPairingCode("");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	async function copyInvitationLink() {
		if (pending() || !session() || !share() || !invitation()) return;
		setPendingAction("copy");
		setError(null);
		setInvitationToast(null);
		try {
			await navigator.clipboard.writeText(invitation());
			showInvitationToast("invitationLinkCopied");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	async function shareInvitationQrCode() {
		const currentShare = share();
		if (pending() || !session() || !currentShare) return;
		setPendingAction("share");
		setError(null);
		setInvitationToast(null);
		try {
			const image = new Image();
			image.decoding = "async";
			await new Promise<void>((resolve, reject) => {
				image.onload = () => resolve();
				image.onerror = () => reject(new Error("QR code image could not be loaded."));
				image.src = currentShare.qr_svg_data_url;
			});
			const canvas = document.createElement("canvas");
			canvas.width = image.naturalWidth || 256;
			canvas.height = image.naturalHeight || 256;
			const context = canvas.getContext("2d");
			if (!context) throw new Error("QR code image could not be created.");
			context.drawImage(image, 0, 0, canvas.width, canvas.height);
			const blob = await new Promise<Blob>((resolve, reject) =>
				canvas.toBlob(
					(value) =>
						value ? resolve(value) : reject(new Error("QR code image could not be created.")),
					"image/png",
				),
			);
			const file = new File([blob], "dukto-invitation-qr.png", { type: "image/png" });
			const shareData = { files: [file], title: t("internetTransferTitle") };
			if (navigator.share && navigator.canShare?.(shareData)) {
				await navigator.share(shareData);
				showInvitationToast("invitationQrShared");
			} else {
				await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
				showInvitationToast("invitationQrCopied");
			}
		} catch (reason) {
			if (reason instanceof DOMException && reason.name === "AbortError") return;
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	async function cancelInvitation() {
		const current = session();
		if (pending() || !current) return;
		setPendingAction("cancel");
		setError(null);
		try {
			await cancelInternetInvite(current.session_id);
			clearSelectedFiles();
			setSession(null);
			setShare(null);
			setInvitationToast(null);
			setInvitation("");
			setPairingCode("");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	async function disconnect() {
		const current = session();
		if (pending() || !current) return;
		setPendingAction("disconnect");
		setError(null);
		try {
			await disconnectInternetSession(current.session_id);
			clearSelectedFiles();
			setSession(null);
			setShare(null);
			setInvitationToast(null);
			setPairingCode("");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPendingAction(null);
		}
	}

	return (
		<>
			<Dialog
				open={showInternetDialog()}
				onOpenChange={(open) => {
					if (!open) closeInternetDialog();
				}}
			>
				<DialogContent aria-busy={pending()} class="max-h-[calc(100vh-2rem)] w-4/5 max-w-none">
					<DialogHeader class="pr-8">
						<div class="flex items-start gap-3 text-left">
							<span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
								<CarbonDirectLink aria-hidden="true" class="size-[22px]" />
							</span>
							<div class="min-w-0 space-y-1">
								<DialogTitle>{t("internetTransferTitle")}</DialogTitle>
								<DialogDescription>{t("internetTransferDescription")}</DialogDescription>
							</div>
						</div>
					</DialogHeader>
					<Show
						when={!isConnected()}
						fallback={
							<p class="text-sm text-muted-foreground">{t("internetPeerAlreadyConnected")}</p>
						}
					>
						<div class="space-y-4">
							<Button
								type="button"
								variant="outline"
								class="w-full"
								disabled={pending() || Boolean(session())}
								onClick={() => void createInvitation()}
							>
								<Show
									when={pendingAction() === "create"}
									fallback={<CarbonLink aria-hidden="true" class="size-4" />}
								>
									<CarbonCircleDash aria-hidden="true" class="size-4 motion-safe:animate-spin" />
								</Show>
								{pendingAction() === "create"
									? t("creatingInternetInvitation")
									: t("createInternetInvitation")}
							</Button>
							<div class="flex flex-col gap-2 sm:flex-row sm:items-end">
								<TextField class="min-w-0 flex-1">
									<TextFieldLabel>{t("remoteInvitationLabel")}</TextFieldLabel>
									<TextFieldInput
										type="text"
										autocomplete="off"
										spellcheck={false}
										readOnly={Boolean(session())}
										value={invitation()}
										onInput={(event) => setInvitation(event.currentTarget.value)}
										onKeyDown={(event) => {
											if (event.key === "Enter") {
												event.preventDefault();
												void connect();
											}
										}}
									/>
								</TextField>
								<Button
									type="button"
									class="w-full sm:w-auto"
									disabled={pending() || Boolean(session()) || !invitation().trim()}
									onClick={() => void connect()}
								>
									<Show
										when={pendingAction() === "connect"}
										fallback={<CarbonDirectLink aria-hidden="true" class="size-4" />}
									>
										<CarbonCircleDash aria-hidden="true" class="size-4 motion-safe:animate-spin" />
									</Show>
									{connecting() ? t("internetPeerConnecting") : t("connectWithInvitation")}
								</Button>
							</div>
							<Show when={share()}>
								{(current) => (
									<div class="flex flex-wrap items-center gap-3 rounded-lg bg-muted/60 p-3">
										<img
											src={current().qr_svg_data_url}
											alt={t("qrInvitationAlt")}
											class="size-32 rounded-md bg-white p-1"
										/>
										<div class="min-w-0 flex-1 space-y-2">
											<p class="text-xs text-muted-foreground">{t("shareInvitationHint")}</p>
											<output
												aria-label={t("pairingCodeLabel")}
												class="block font-mono text-lg font-semibold tracking-[0.2em] text-foreground"
											>
												{current().manual_code}
											</output>
											<div class="flex flex-wrap items-center gap-2">
												<Button
													type="button"
													variant="outline"
													disabled={pending()}
													onClick={() => void copyInvitationLink()}
												>
													<CarbonCopy aria-hidden="true" class="size-4" />
													{t("copyInvitationLink")}
												</Button>
												<Button
													type="button"
													variant="outline"
													disabled={pending()}
													onClick={() => void shareInvitationQrCode()}
												>
													<Show
														when={pendingAction() === "share"}
														fallback={<CarbonShare aria-hidden="true" class="size-4" />}
													>
														<CarbonCircleDash
															aria-hidden="true"
															class="size-4 motion-safe:animate-spin"
														/>
													</Show>
													{t("shareInvitationQrCode")}
												</Button>
												<Button
													type="button"
													variant="outline"
													disabled={pending()}
													onClick={() => void cancelInvitation()}
												>
													<CarbonClose aria-hidden="true" class="size-4" />
													{t("cancelInvitation")}
												</Button>
											</div>
										</div>
									</div>
								)}
							</Show>
							<Show when={session()?.status.state === "invited" && !share()}>
								<div class="flex flex-col gap-2 rounded-lg bg-muted/60 p-3 sm:flex-row sm:items-end">
									<TextField class="min-w-0 flex-1">
										<TextFieldLabel>{t("pairingCodeLabel")}</TextFieldLabel>
										<TextFieldInput
											type="password"
											inputmode="numeric"
											autocomplete="off"
											maxlength={8}
											value={pairingCode()}
											onInput={(event) =>
												setPairingCode(event.currentTarget.value.replace(/\D/g, "").slice(0, 8))
											}
											onKeyDown={(event) => {
												if (event.key === "Enter") {
													event.preventDefault();
													void confirmPairingCode();
												}
											}}
										/>
									</TextField>
									<Button
										type="button"
										class="w-full sm:w-auto"
										disabled={pending() || !/^\d{8}$/.test(pairingCode())}
										onClick={() => void confirmPairingCode()}
									>
										<Show when={connecting()}>
											<CarbonCircleDash
												aria-hidden="true"
												class="size-4 motion-safe:animate-spin"
											/>
										</Show>
										{connecting() ? t("internetPeerConnecting") : t("confirmPairingCode")}
									</Button>
								</div>
							</Show>
							<Show when={session()}>
								{(current) => (
									<Show when={current().status.state !== "invited"}>
										<RemoteSessionStatus
											status={current().status}
											expiresAtUnix={current().expires_at_unix}
										/>
									</Show>
								)}
							</Show>
						</div>
					</Show>
				</DialogContent>
			</Dialog>
			<Show when={visibleRemotePeer()}>
				{(peer) => (
					<PeerCard
						peer={peer()}
						badge={routeLabel()}
						badgeTitle={routeTitle()}
						transfers={isConnected() ? props.getTransfers?.(peer().device_id) : undefined}
						actionsDisabled={pending() || connecting() || !isConnected()}
						onAddFiles={session()?.can_send ? () => void pickItems(false) : undefined}
						onAddFolders={
							session()?.can_send && supportsFolderSelection(currentPlatform)
								? () => void pickItems(true)
								: undefined
						}
						onAddMedia={
							session()?.can_send && supportsMediaSelection(currentPlatform)
								? () => void pickItems(false, true)
								: undefined
						}
						onDisconnect={isConnected() ? () => void disconnect() : undefined}
						onAbortTransfer={props.onAbortTransfer}
						onDismissTransfer={props.onDismissTransfer}
						expandedContent={
							selectedFiles().length > 0 ? (
								<div aria-busy={pending()}>
									<SendPreview
										embedded
										peer={peer()}
										files={selectedFiles()}
										onConfirm={() => void sendSelectedFiles()}
										onCancel={clearSelectedFiles}
										onRemoveFile={removeSelectedFile}
									/>
								</div>
							) : undefined
						}
					/>
				)}
			</Show>
			<Toast open={invitationToast() !== null}>
				{invitationToast() ? t(invitationToast() as InvitationToast) : null}
			</Toast>
			<Toast open={Boolean(error())}>{localizedError()}</Toast>
		</>
	);
}

export default RemotePeerEntry;
