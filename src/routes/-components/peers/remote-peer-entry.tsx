import { listen } from "@tauri-apps/api/event";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";
import { platform } from "@tauri-apps/plugin-os";
import { Show, createSignal, onCleanup, onMount } from "solid-js";
import { Button } from "~/components/ui/button";
import { TextField, TextFieldInput, TextFieldLabel } from "~/components/ui/text-field";
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
	getInternetInvitationLink,
	importInternetInvite,
	releaseSelectedFiles,
	sendToInternetSession,
} from "~/helpers/tauri";
import SendPreview from "~/routes/-components/transfers/send-preview";
import { startSendTransfer } from "~/routes/-stores/transfers";
import LucideGlobe2 from "~icons/lucide/globe-2";
import RemoteSessionStatus from "./remote-session-status";

function RemotePeerEntry() {
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
	const [linkCopied, setLinkCopied] = createSignal(false);
	const [pending, setPending] = createSignal(false);
	const [selectedFiles, setSelectedFiles] = createSignal<FileMetadataInfo[]>([]);
	const [error, setError] = createSignal<string | null>(null);
	const localizedError = () => {
		const message = error();
		return message ? t(nativeErrorKey(message)) : null;
	};

	async function consumeDeepLinks(urls: string[] | null) {
		for (const url of urls ?? []) {
			if (!url.startsWith("dukto://connect#")) continue;
			try {
				clearSelectedFiles();
				setSession(await importInternetInvite(url));
				setShare(null);
				setLinkCopied(false);
				setInvitation("");
				setPairingCode("");
				setError(null);
			} catch (reason) {
				setError(String(reason));
			}
		}
	}

	async function confirmPairingCode() {
		const current = session();
		const code = pairingCode().trim();
		if (pending() || !current || !/^\d{8}$/.test(code)) return;
		setPending(true);
		setError(null);
		try {
			setSession(await confirmInternetPairingCode(current.session_id, code));
			setPairingCode("");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPending(false);
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
			setSession(payload);
			if (payload.status.state !== "invited" && payload.status.state !== "pairing") {
				setShare(null);
				setLinkCopied(false);
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
		});
	});

	function releaseFiles(files: FileMetadataInfo[]) {
		void releaseSelectedFiles(files.map((file) => file.path)).catch(() => undefined);
	}

	async function pickItems(directory: boolean, media = false) {
		const current = session();
		if (pending() || current?.status.state !== "ready" || !current.can_send || share()) return;
		setPending(true);
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
			setPending(false);
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
		setPending(true);
		setError(null);
		try {
			const transferId = await sendToInternetSession(
				current.session_id,
				files.map((file) => file.path),
			);
			const totalSize = files.reduce((sum, file) => sum + file.size, 0);
			startSendTransfer(transferId, totalSize, current.peer_id, {
				device_id: current.peer_id,
				display_name: current.peer_id.slice(0, 8),
				hostname: "internet",
				platform: "unknown",
			});
			setSelectedFiles([]);
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPending(false);
		}
	}

	async function createInvitation() {
		if (pending()) return;
		setPending(true);
		setError(null);
		try {
			clearSelectedFiles();
			const created = await createInternetInvite();
			setSession(created);
			setShare(created);
			setLinkCopied(false);
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPending(false);
		}
	}

	async function connect() {
		const value = invitation().trim();
		if (pending() || !value) return;
		setPending(true);
		setError(null);
		try {
			clearSelectedFiles();
			setSession(await importInternetInvite(value));
			setShare(null);
			setLinkCopied(false);
			setInvitation("");
			setPairingCode("");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPending(false);
		}
	}

	async function copyInvitationLink() {
		const current = session();
		if (pending() || !current || !share()) return;
		setPending(true);
		setError(null);
		setLinkCopied(false);
		let invitationLink = "";
		try {
			invitationLink = await getInternetInvitationLink(current.session_id);
			await navigator.clipboard.writeText(invitationLink);
			setLinkCopied(true);
		} catch (reason) {
			setError(String(reason));
		} finally {
			invitationLink = "";
			setPending(false);
		}
	}

	async function cancel() {
		const current = session();
		if (pending() || !current) return;
		setPending(true);
		setError(null);
		try {
			await cancelInternetInvite(current.session_id);
			clearSelectedFiles();
			setSession({ ...current, status: { state: "cancelled" } });
			setShare(null);
			setLinkCopied(false);
			setInvitation("");
			setPairingCode("");
		} catch (reason) {
			setError(String(reason));
		} finally {
			setPending(false);
		}
	}

	return (
		<section
			aria-labelledby="internet-transfer-title"
			aria-busy={pending()}
			class="shrink-0 space-y-3 rounded-xl border border-dashed border-border bg-card/70 p-3.5"
		>
			<div class="flex flex-col gap-3 sm:flex-row sm:items-start">
				<span class="flex size-10 shrink-0 items-center justify-center rounded-lg bg-muted text-muted-foreground">
					<LucideGlobe2 aria-hidden="true" class="size-[22px]" />
				</span>
				<div class="min-w-0 flex-1">
					<h2 id="internet-transfer-title" class="text-sm font-semibold">
						{t("internetTransferTitle")}
					</h2>
					<p class="text-xs text-muted-foreground">{t("internetTransferDescription")}</p>
				</div>
				<Button
					type="button"
					variant="outline"
					class="w-full sm:w-auto"
					disabled={pending()}
					onClick={() => void createInvitation()}
				>
					{t("createInternetInvitation")}
				</Button>
			</div>

			<div class="flex flex-col gap-2 sm:flex-row sm:items-end">
				<TextField class="min-w-0 flex-1">
					<TextFieldLabel>{t("remoteInvitationLabel")}</TextFieldLabel>
					<TextFieldInput
						type="text"
						autocomplete="off"
						spellcheck={false}
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
					disabled={pending() || !invitation().trim()}
					onClick={() => void connect()}
				>
					{t("connectWithInvitation")}
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
									{t("copyInvitationLink")}
								</Button>
								<Button
									type="button"
									variant="ghost"
									disabled={pending()}
									onClick={() => void cancel()}
								>
									{t("cancelInvitation")}
								</Button>
							</div>
							<Show when={linkCopied()}>
								<output aria-live="polite" class="text-xs text-muted-foreground">
									{t("invitationLinkCopied")}
								</output>
							</Show>
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
						{t("confirmPairingCode")}
					</Button>
				</div>
			</Show>

			<Show when={session()?.status.state === "ready" && session()?.can_send && !share()}>
				<div class="space-y-2 rounded-lg bg-muted/60 p-3">
					<div class="flex flex-wrap gap-2">
						<Button
							type="button"
							variant="outline"
							disabled={pending()}
							onClick={() => void pickItems(false)}
						>
							{t("addFiles")}
						</Button>
						<Show when={supportsFolderSelection(currentPlatform)}>
							<Button
								type="button"
								variant="outline"
								disabled={pending()}
								onClick={() => void pickItems(true)}
							>
								{t("addFolders")}
							</Button>
						</Show>
						<Show when={supportsMediaSelection(currentPlatform)}>
							<Button
								type="button"
								variant="outline"
								disabled={pending()}
								onClick={() => void pickItems(false, true)}
							>
								{t("addPhotosAndVideos")}
							</Button>
						</Show>
					</div>
					<Show when={selectedFiles().length > 0}>
						<SendPreview
							embedded
							peer={{
								display_name: session()?.peer_id.slice(0, 8) ?? "",
								hostname: "internet",
								platform: "unknown",
							}}
							files={selectedFiles()}
							onConfirm={() => void sendSelectedFiles()}
							onCancel={clearSelectedFiles}
							onRemoveFile={removeSelectedFile}
						/>
					</Show>
				</div>
			</Show>

			<Show
				when={session()}
				fallback={
					<Show when={error()}>
						{(message) => <p role="alert">{t(nativeErrorKey(message()))}</p>}
					</Show>
				}
			>
				{(current) => (
					<RemoteSessionStatus
						status={current().status}
						expiresAtUnix={current().expires_at_unix}
						error={localizedError()}
					/>
				)}
			</Show>
		</section>
	);
}

export default RemotePeerEntry;
