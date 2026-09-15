import { Match, Show, Switch } from "solid-js";
import { Button } from "~/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "~/components/ui/dialog";
import { getUpdateDownloadLabelKey } from "~/helpers/app-update-controller";
import { t } from "~/helpers/i18n";
import { appUpdates } from "~/stores/app-updates";
import { formatBytes } from "~/utils/format";

function UpdateAvailableDialog() {
	const state = appUpdates.state;
	const isLocked = () => state.status === "restartFailed";
	const closeAllowed = () => !appUpdates.isBusy() && !isLocked();
	const errorMessage = () => {
		if (state.errorCode === "transferGate") return t("updateWaitingForTransfers");
		if (state.errorCode === "download") {
			return t("updateDownloadFailedWithDetails", { message: state.errorMessage ?? "" });
		}
		if (state.errorCode === "install") {
			return t("updateInstallFailedWithDetails", { message: state.errorMessage ?? "" });
		}
		if (state.errorCode === "restart") return t("updateRestartManually");
		return null;
	};
	const progressLabel = () => {
		const progress = state.progress;
		if (!progress) return "";
		const downloaded = formatBytes(progress.downloaded);
		return progress.total === null
			? t("updateDownloadProgressIndeterminate", { downloaded })
			: t("updateDownloadProgress", { downloaded, total: formatBytes(progress.total) });
	};

	return (
		<Dialog open={state.dialogOpen} onOpenChange={(open) => !open && appUpdates.dismissUpdate()}>
			<DialogContent
				class="flex max-h-[calc(100%-2rem)] w-[calc(100%-2rem)] flex-col gap-3 overflow-hidden rounded-lg p-5"
				overlayClass="bg-black/40"
				showCloseButton={closeAllowed()}
				onEscapeKeyDown={(event: Event) => event.preventDefault()}
				onInteractOutside={(event: Event) => event.preventDefault()}
			>
				<div class="shrink-0 space-y-1 pr-6">
					<DialogTitle class="text-sm font-semibold">
						{t("updateAvailableTitle", { version: state.availableVersion ?? "" })}
					</DialogTitle>
					<DialogDescription>
						{t("currentAndNewVersion", {
							current: state.currentVersion ?? "...",
							next: state.availableVersion ?? "",
						})}
					</DialogDescription>
				</div>
				<p class="shrink-0 text-xs font-medium text-muted-foreground">{t("releaseNotes")}</p>
				<div class="min-h-0 overflow-y-auto rounded-md border border-border bg-muted/40 p-3 text-xs leading-relaxed">
					<Show
						when={state.releaseNotes}
						fallback={<p class="text-muted-foreground">{t("updateNoReleaseNotes")}</p>}
					>
						<p class="whitespace-pre-wrap">{state.releaseNotes}</p>
					</Show>
				</div>
				<div class="shrink-0 space-y-3">
					<Show when={state.progress}>
						{(progress) => (
							<output class="block space-y-1.5">
								<div class="flex justify-between gap-2 text-xs text-muted-foreground">
									<span>{t(getUpdateDownloadLabelKey(state.status))}</span>
									<span>{progressLabel()}</span>
								</div>
								<div
									class="h-1.5 overflow-hidden rounded-full bg-muted"
									role="progressbar"
									tabIndex={0}
									aria-label={t("updateDownloadProgressLabel")}
									aria-valuemin={0}
									aria-valuemax={100}
									aria-valuenow={progress().percent ?? undefined}
								>
									<div
										class={`h-full rounded-full bg-primary ${progress().percent === null ? "animate-pulse" : ""}`}
										style={{ width: `${progress().percent ?? 28}%` }}
									/>
								</div>
							</output>
						)}
					</Show>
					<Show when={errorMessage()}>
						{(message) => (
							<p class="text-xs text-destructive" role="alert">
								{message()}
							</p>
						)}
					</Show>
					<Switch>
						<Match when={state.status === "preparingInstall"}>
							<output class="text-xs text-muted-foreground">{t("updatePreparingInstall")}</output>
						</Match>
						<Match when={state.status === "installing"}>
							<output class="text-xs text-muted-foreground">{t("updateInstalling")}</output>
						</Match>
						<Match when={state.status === "restarting"}>
							<output class="text-xs text-muted-foreground">{t("updateRestarting")}</output>
						</Match>
						<Match when={state.status === "restartFailed"}>
							<p class="text-xs text-destructive" role="alert">
								{t("updateRestartManually")}
							</p>
						</Match>
					</Switch>
					<div class="flex justify-end gap-2">
						<Switch>
							<Match when={state.status === "available" || state.status === "downloadError"}>
								<Button variant="outline" onClick={appUpdates.dismissUpdate}>
									{t("later")}
								</Button>
								<Button onClick={() => void appUpdates.downloadUpdate()}>
									{state.status === "downloadError" ? t("retryDownload") : t("downloadUpdate")}
								</Button>
							</Match>
							<Match
								when={
									state.status === "downloaded" ||
									state.status === "waitingForTransfers" ||
									state.status === "installError"
								}
							>
								<Button variant="outline" onClick={appUpdates.dismissUpdate}>
									{t("later")}
								</Button>
								<Button onClick={() => void appUpdates.installUpdate()}>
									{state.status === "downloaded" ? t("installAndRestart") : t("retryInstall")}
								</Button>
							</Match>
						</Switch>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

export default UpdateAvailableDialog;
