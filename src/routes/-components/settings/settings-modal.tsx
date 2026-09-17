import { open } from "@tauri-apps/plugin-shell";
import { Show } from "solid-js";
import { Button } from "~/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "~/components/ui/dialog";
import { Tabs, TabsList, TabsTrigger } from "~/components/ui/tabs";
import { TextField, TextFieldInput } from "~/components/ui/text-field";
import type { AppUpdateErrorCode, AppUpdateStatus } from "~/helpers/app-update-controller";
import { t } from "~/helpers/i18n";
import { nativeErrorKey } from "~/helpers/native-error";
import LucideExternalLink from "~icons/lucide/external-link";
import LucideRefreshCw from "~icons/lucide/refresh-cw";

const PRIVACY_POLICY_URL = "https://dukto.app/docs/privacidade/";

export type ThemeMode = "light" | "dark" | "system";

interface SettingsModalProps {
	open: boolean;
	destinationDir: string;
	theme: ThemeMode;
	language?: string;
	currentVersion: string | null;
	availableVersion: string | null;
	updateBusy: boolean;
	updateErrorCode: AppUpdateErrorCode;
	updateErrorMessage: string | null;
	updateStatus: AppUpdateStatus;
	showAppUpdates: boolean;
	onChangeDestination: () => void;
	onChangeTheme: (theme: ThemeMode) => void;
	onChangeLanguage?: (lang: string) => void;
	onCheckForUpdates: () => void;
	onClose: () => void;
}

function SettingsModal(props: SettingsModalProps) {
	const openPrivacyPolicy = () => {
		void open(PRIVACY_POLICY_URL).catch((error) => {
			console.error("Could not open privacy policy", error);
		});
	};

	const statusMessage = () => {
		switch (props.updateStatus) {
			case "checking":
				return t("checkingForUpdates");
			case "upToDate":
				return t("upToDate");
			case "available":
			case "downloadError":
				return t("updateAvailableVersion", { version: props.availableVersion ?? "" });
			case "downloading":
				return t("updateDownloading");
			case "downloaded":
				return t("updateDownloaded");
			case "preparingInstall":
				return t("updatePreparingInstall");
			case "waitingForTransfers":
				return t("updateWaitingForTransfers");
			case "installing":
				return t("updateInstalling");
			case "installError":
				return t("updateInstallFailed");
			case "restarting":
				return t("updateRestarting");
			case "restartFailed":
				return t("updateRestartManually");
			default:
				return t("updateNotChecked");
		}
	};

	const errorMessage = () => {
		if (props.updateErrorCode === "feedUnavailable") return t("updateFeedUnavailable");
		if (props.updateErrorCode === "check") {
			return t("updateCheckFailedWithDetails", {
				message: t(nativeErrorKey(props.updateErrorMessage)),
			});
		}
		if (props.updateErrorCode === "download") {
			return t("updateDownloadFailedWithDetails", {
				message: t(nativeErrorKey(props.updateErrorMessage)),
			});
		}
		if (props.updateErrorCode === "install") {
			return t("updateInstallFailedWithDetails", {
				message: t(nativeErrorKey(props.updateErrorMessage)),
			});
		}
		return null;
	};

	return (
		<Dialog open={props.open} onOpenChange={(open) => !open && props.onClose()}>
			<DialogContent
				class="max-h-[calc(100dvh-2rem)] w-[calc(100%-2rem)] max-w-xs gap-4 rounded-lg p-5"
				overlayClass="bg-black/40"
				onEscapeKeyDown={(event: Event) => event.preventDefault()}
			>
				<DialogTitle class="text-sm font-semibold">{t("settings")}</DialogTitle>
				<div class="space-y-3">
					<div class="space-y-1">
						<p class="text-xs font-medium text-muted-foreground">{t("saveFilesTo")}</p>
						<div class="flex items-center gap-1.5">
							<TextField class="min-w-0 flex-1">
								<TextFieldInput disabled value={props.destinationDir} class="bg-muted" />
							</TextField>
							<Button variant="outline" onClick={props.onChangeDestination}>
								{t("change")}
							</Button>
						</div>
					</div>
					<div class="space-y-1">
						<p class="text-xs font-medium text-muted-foreground">{t("appearance")}</p>
						<Tabs value={props.theme} onChange={(value) => props.onChangeTheme(value as ThemeMode)}>
							<TabsList class="h-8">
								<TabsTrigger value="light" class="capitalize">
									{t("light")}
								</TabsTrigger>
								<TabsTrigger value="dark" class="capitalize">
									{t("dark")}
								</TabsTrigger>
								<TabsTrigger value="system" class="capitalize">
									{t("system")}
								</TabsTrigger>
							</TabsList>
						</Tabs>
					</div>
					<Show when={props.onChangeLanguage}>
						<div class="space-y-1">
							<p class="text-xs font-medium text-muted-foreground">{t("language")}</p>
							<Tabs
								value={props.language ?? "pt"}
								onChange={(value) => props.onChangeLanguage?.(value)}
							>
								<TabsList class="h-8">
									<TabsTrigger value="en">English</TabsTrigger>
									<TabsTrigger value="pt">Português</TabsTrigger>
									<TabsTrigger value="es">Español</TabsTrigger>
								</TabsList>
							</Tabs>
						</div>
					</Show>
					<footer class="space-y-1.5 border-t border-border pt-3" aria-label={t("about")}>
						<div class="flex items-center justify-between gap-2">
							<div class="flex min-w-0 items-center gap-1">
								<span
									class="shrink-0 text-xs text-muted-foreground"
									aria-label={t("currentVersion", { version: props.currentVersion ?? "..." })}
								>
									v{props.currentVersion ?? "..."}
								</span>
								<Button
									variant="ghost"
									class="h-6 min-w-0 gap-1 px-1.5 text-xs font-normal text-muted-foreground hover:text-foreground [&_svg]:size-3"
									onClick={openPrivacyPolicy}
								>
									{t("privacyPolicy")}
									<LucideExternalLink class="size-3 shrink-0" />
								</Button>
							</div>
							<Show when={props.showAppUpdates}>
								<Button
									variant="ghost"
									class="h-6 min-w-0 gap-1.5 px-1.5 text-xs font-normal text-muted-foreground hover:text-foreground [&_svg]:size-3"
									disabled={props.updateBusy}
									onClick={props.onCheckForUpdates}
								>
									<LucideRefreshCw
										class="size-3 shrink-0"
										classList={{ "motion-safe:animate-spin": props.updateStatus === "checking" }}
									/>
									{props.availableVersion ? t("viewUpdate") : t("checkForUpdates")}
								</Button>
							</Show>
						</div>
						<Show
							when={
								props.showAppUpdates &&
								props.updateStatus !== "idle" &&
								props.updateStatus !== "checking" &&
								!errorMessage()
							}
						>
							<output class="block text-xs leading-relaxed text-muted-foreground">
								{statusMessage()}
							</output>
						</Show>
						<Show when={errorMessage()}>
							{(message) => (
								<output class="block break-words text-xs leading-relaxed text-destructive">
									{message()}
								</output>
							)}
						</Show>
					</footer>
				</div>
			</DialogContent>
		</Dialog>
	);
}

export default SettingsModal;
