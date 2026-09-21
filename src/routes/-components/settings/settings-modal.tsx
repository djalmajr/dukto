import { Show, createEffect, createSignal, onCleanup } from "solid-js";
import { Button } from "~/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "~/components/ui/dialog";
import { Tabs, TabsList, TabsTrigger } from "~/components/ui/tabs";
import { TextField, TextFieldInput } from "~/components/ui/text-field";
import Toast from "~/components/ui/toast";
import type { AppUpdateErrorCode, AppUpdateStatus } from "~/helpers/app-update-controller";
import { t } from "~/helpers/i18n";
import { nativeErrorKey } from "~/helpers/native-error";
import { openPrivacyPolicy } from "~/helpers/privacy-policy";
import CarbonLaunch from "~icons/carbon/launch";
import CarbonRenew from "~icons/carbon/renew";

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
	onCheckForUpdates: () => Promise<void>;
	onClose: () => void;
}

function SettingsModal(props: SettingsModalProps) {
	const [displayName, setDisplayName] = createSignal(props.displayName);
	const [savingDisplayName, setSavingDisplayName] = createSignal(false);
	const [displayNameError, setDisplayNameError] = createSignal<string | null>(null);
	const [changingDestination, setChangingDestination] = createSignal(false);
	const [destinationError, setDestinationError] = createSignal<string | null>(null);
	const [privacyError, setPrivacyError] = createSignal(false);
	const [upToDateToastVisible, setUpToDateToastVisible] = createSignal(false);
	let toastTimer: ReturnType<typeof setTimeout> | undefined;

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

	const handleCheckForUpdates = async () => {
		await props.onCheckForUpdates();
		if (props.updateStatus === "upToDate") {
			setUpToDateToastVisible(true);
			if (toastTimer) clearTimeout(toastTimer);
			toastTimer = setTimeout(() => setUpToDateToastVisible(false), 3500);
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

	onCleanup(() => {
		if (toastTimer) clearTimeout(toastTimer);
	});

	return (
		<>
			<Dialog open={props.open} onOpenChange={(open) => !open && props.onClose()}>
				<DialogContent
					class="max-h-[calc(100dvh-2rem)] w-4/5 max-w-none gap-4 rounded-lg p-5"
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
							<Tabs
								value={props.theme}
								onChange={(value) => props.onChangeTheme(value as ThemeMode)}
							>
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
						<Show when={props.showAppUpdates}>
							<div class="space-y-1.5">
								<Button
									variant="outline"
									class="w-full gap-1.5"
									disabled={props.updateBusy}
									onClick={() => void handleCheckForUpdates()}
								>
									<CarbonRenew
										class="size-4"
										classList={{ "motion-safe:animate-spin": props.updateStatus === "checking" }}
									/>
									{props.updateStatus === "checking"
										? t("checkingForUpdates")
										: props.availableVersion
											? t("viewUpdate")
											: t("checkForUpdates")}
								</Button>
								<Show when={errorMessage()}>
									{(message) => (
										<output class="block break-words text-xs leading-relaxed text-destructive">
											{message()}
										</output>
									)}
								</Show>
							</div>
						</Show>
						<footer class="space-y-1.5" aria-label={t("about")}>
							<div class="flex items-center justify-between gap-3">
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
									<CarbonLaunch class="size-3.5" />
								</Button>
							</div>
							<Show when={privacyError()}>
								<output class="block text-xs leading-relaxed text-destructive">
									{t("operationFailed")}
								</output>
							</Show>
						</footer>
					</div>
				</DialogContent>
			</Dialog>
			<Toast open={upToDateToastVisible()}>{t("upToDate")}</Toast>
		</>
	);
}

export default SettingsModal;
