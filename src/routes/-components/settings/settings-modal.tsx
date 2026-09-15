import { Show } from "solid-js";
import { Button } from "~/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "~/components/ui/dialog";
import { Tabs, TabsList, TabsTrigger } from "~/components/ui/tabs";
import { TextField, TextFieldInput } from "~/components/ui/text-field";
import type { AppUpdateErrorCode, AppUpdateStatus } from "~/helpers/app-update-controller";
import { t } from "~/helpers/i18n";

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
	onChangeDestination: () => void;
	onChangeTheme: (theme: ThemeMode) => void;
	onChangeLanguage?: (lang: string) => void;
	onCheckForUpdates: () => void;
	onClose: () => void;
}

function SettingsModal(props: SettingsModalProps) {
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
			return t("updateCheckFailedWithDetails", { message: props.updateErrorMessage ?? "" });
		}
		if (props.updateErrorCode === "download") {
			return t("updateDownloadFailedWithDetails", { message: props.updateErrorMessage ?? "" });
		}
		if (props.updateErrorCode === "install") {
			return t("updateInstallFailedWithDetails", { message: props.updateErrorMessage ?? "" });
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
					<div class="space-y-1">
						<p class="text-xs font-medium text-muted-foreground">{t("appUpdates")}</p>
						<div class="flex items-center justify-between gap-2">
							<div class="min-w-0 space-y-0.5">
								<p class="text-xs">
									{t("currentVersion", { version: props.currentVersion ?? "..." })}
								</p>
								<p class="text-xs text-muted-foreground">{statusMessage()}</p>
							</div>
							<Button
								variant="outline"
								disabled={props.updateBusy}
								onClick={props.onCheckForUpdates}
							>
								{props.updateStatus === "checking"
									? t("checkingForUpdates")
									: props.availableVersion
										? t("viewUpdate")
										: t("checkForUpdates")}
							</Button>
						</div>
						<Show when={errorMessage()}>
							{(message) => <p class="text-xs text-destructive">{message()}</p>}
						</Show>
					</div>
				</div>
			</DialogContent>
		</Dialog>
	);
}

export default SettingsModal;
