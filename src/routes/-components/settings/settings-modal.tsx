import { Show, createEffect, createSignal } from "solid-js";
import { Button } from "~/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "~/components/ui/dialog";
import { Tabs, TabsList, TabsTrigger } from "~/components/ui/tabs";
import { TextField, TextFieldInput } from "~/components/ui/text-field";
import type { AppUpdateErrorCode, AppUpdateStatus } from "~/helpers/app-update-controller";
import { t } from "~/helpers/i18n";
import { nativeErrorKey } from "~/helpers/native-error";
import { openPrivacyPolicy } from "~/helpers/privacy-policy";
import LucideExternalLink from "~icons/lucide/external-link";
import LucideRefreshCw from "~icons/lucide/refresh-cw";

export type ThemeMode = "light" | "dark" | "system";

interface SettingsModalProps {
	open: boolean;
	displayName: string;
	destinationDir: string;
	theme: ThemeMode;
	language?: string;
	currentVersion: string | null;
	availableVersion: string | null;
	currentPlatform: string;
	updateBusy: boolean;
	updateErrorCode: AppUpdateErrorCode;
	updateErrorMessage: string | null;
	updateStatus: AppUpdateStatus;
	showAppUpdates: boolean;
	onChangeDisplayName: (displayName: string) => Promise<void>;
	onChangeDestination: () => Promise<void>;
	onChangeTheme: (theme: ThemeMode) => void;
	onChangeLanguage?: (lang: string) => void;
	onCheckForUpdates: () => void;
	onClose: () => void;
}

function SettingsModal(props: SettingsModalProps) {
	const [displayName, setDisplayName] = createSignal(props.displayName);
	const [savingDisplayName, setSavingDisplayName] = createSignal(false);
	const [displayNameError, setDisplayNameError] = createSignal<string | null>(null);
	const [changingDestination, setChangingDestination] = createSignal(false);
	const [destinationError, setDestinationError] = createSignal<string | null>(null);
	const [privacyError, setPrivacyError] = createSignal(false);

	createEffect(() => {
		if (!props.open) return;
		setDisplayName(props.displayName);
		setDisplayNameError(null);
		setDestinationError(null);
		setPrivacyError(false);
	});

	const saveDisplayName = async () => {
		const nextName = displayName().trim();
		if (!nextName) {
			setDisplayNameError(t("userNameRequired"));
			return;
		}
		setSavingDisplayName(true);
		setDisplayNameError(null);
		try {
			await props.onChangeDisplayName(nextName);
			setDisplayName(nextName);
		} catch {
			setDisplayNameError(t("userNameSaveFailed"));
		} finally {
			setSavingDisplayName(false);
		}
	};

	const changeDestination = async () => {
		setChangingDestination(true);
		setDestinationError(null);
		try {
			await props.onChangeDestination();
		} catch {
			setDestinationError(t("destinationFolderChangeFailed"));
		} finally {
			setChangingDestination(false);
		}
	};

	const handleOpenPrivacyPolicy = () => {
		setPrivacyError(false);
		void openPrivacyPolicy(props.currentPlatform).catch(() => setPrivacyError(true));
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
						<p class="text-xs font-medium text-muted-foreground">{t("userName")}</p>
						<div class="flex items-center gap-1.5">
							<TextField class="min-w-0 flex-1">
								<TextFieldInput
									value={displayName()}
									maxlength={64}
									onInput={(event) => setDisplayName(event.currentTarget.value)}
									onKeyDown={(event) => {
										if (event.key === "Enter") {
											event.preventDefault();
											void saveDisplayName();
										}
									}}
								/>
							</TextField>
							<Button
								variant="outline"
								disabled={
									savingDisplayName() ||
									!displayName().trim() ||
									displayName().trim() === props.displayName
								}
								onClick={() => void saveDisplayName()}
							>
								{savingDisplayName() ? t("saving") : t("save")}
							</Button>
						</div>
						<Show when={displayNameError()}>
							{(message) => <output class="block text-xs text-destructive">{message()}</output>}
						</Show>
					</div>
					<div class="space-y-1">
						<p class="text-xs font-medium text-muted-foreground">{t("saveFilesTo")}</p>
						<div class="flex items-center gap-1.5">
							<TextField class="min-w-0 flex-1">
								<TextFieldInput disabled value={props.destinationDir} class="bg-muted" />
							</TextField>
							<Button
								variant="outline"
								disabled={changingDestination()}
								onClick={() => void changeDestination()}
							>
								{changingDestination() ? t("saving") : t("change")}
							</Button>
						</div>
						<Show when={destinationError()}>
							{(message) => <output class="block text-xs text-destructive">{message()}</output>}
						</Show>
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
						<div class="flex flex-wrap items-center gap-1">
							<span
								class="shrink-0 whitespace-nowrap text-xs text-muted-foreground"
								aria-label={t("currentVersion", { version: props.currentVersion ?? "..." })}
							>
								v{props.currentVersion ?? "..."}
							</span>
							<Button
								variant="ghost"
								class="h-6 shrink-0 gap-1 px-1.5 text-xs font-normal text-muted-foreground hover:text-foreground"
								onClick={handleOpenPrivacyPolicy}
							>
								{t("privacyPolicy")}
								<LucideExternalLink class="size-4" />
							</Button>
							<Show when={props.showAppUpdates}>
								<Button
									variant="ghost"
									class="ml-auto h-6 shrink-0 gap-1.5 px-1.5 text-xs font-normal text-muted-foreground hover:text-foreground"
									disabled={props.updateBusy}
									onClick={props.onCheckForUpdates}
								>
									<LucideRefreshCw
										class="size-4"
										classList={{ "motion-safe:animate-spin": props.updateStatus === "checking" }}
									/>
									{props.availableVersion ? t("viewUpdate") : t("checkForUpdates")}
								</Button>
							</Show>
						</div>
						<Show when={privacyError()}>
							<output class="block text-xs leading-relaxed text-destructive">
								{t("operationFailed")}
							</output>
						</Show>
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
