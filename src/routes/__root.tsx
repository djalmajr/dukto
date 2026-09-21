import { Outlet, createRootRoute, useNavigate, useSearch } from "@tanstack/solid-router";
import { type UnlistenFn, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { sendNotification } from "@tauri-apps/plugin-notification";
import { platform } from "@tauri-apps/plugin-os";
import { Show, createEffect, onCleanup, onMount } from "solid-js";
import { Button } from "~/components/ui/button";
import WindowControls from "~/components/window-controls";
import { pickDestinationDirectory } from "~/helpers/destination-directory";
import { changeLanguage, language, t } from "~/helpers/i18n";
import SettingsModal from "~/routes/-components/settings/settings-modal";
import UpdateAvailableDialog from "~/routes/-components/settings/update-available-dialog";
import { type IncomingRequest, incomingRequest } from "~/routes/-stores/transfers";
import { appUpdates } from "~/stores/app-updates";
import { device, setDeviceDisplayName } from "~/stores/device";
import {
	resolvedTheme,
	setDestinationDir,
	setTheme,
	settings,
	theme,
	updateDisplayName,
} from "~/stores/settings";
import { formatDeviceHostname } from "~/utils/device-hostname";
import LucideCirclePlus from "~icons/lucide/circle-plus";
import LucideSettings from "~icons/lucide/settings";

function isInteractiveTarget(target: HTMLElement) {
	return target.closest("button,[role='button'],a,input,select,textarea,[data-no-window-drag]");
}

function RootLayout() {
	const currentPlatform = (() => {
		try {
			return platform();
		} catch {
			if (typeof navigator !== "undefined") {
				if (navigator.platform.startsWith("Win")) return "windows";
				if (navigator.platform.startsWith("Mac")) return "macos";
				if (navigator.platform.startsWith("iPhone") || navigator.platform.startsWith("iPad"))
					return "ios";
			}
			return "linux";
		}
	})();
	const isWindows = currentPlatform === "windows";
	const isMac = currentPlatform === "macos";
	const isMobile = currentPlatform === "android" || currentPlatform === "ios";
	const navigate = useNavigate();
	const search = useSearch({ strict: false });
	const showSettings = () =>
		(search() as { internet?: boolean; settings?: boolean }).settings === true;
	const showInternetPeer = () =>
		(search() as { internet?: boolean; settings?: boolean }).internet === true;

	let stopNotifications: UnlistenFn | undefined;
	let disposed = false;
	onMount(async () => {
		void appUpdates.startupCheck();
		const stop = await listen<IncomingRequest>("transfer:incoming", ({ payload }) => {
			if (disposed) return;
			const sender = payload.sender;
			const name = sender
				? [sender.display_name, formatDeviceHostname(sender.hostname)].filter(Boolean).join("@")
				: t("anotherDevice");
			try {
				sendNotification({
					title: t("incoming"),
					body: t("incomingNotification", { count: payload.item_count, sender: name }),
				});
			} catch (error) {
				console.error("Could not show incoming notification", error);
			}
		});
		if (disposed) stop();
		else stopNotifications = stop;
	});
	onCleanup(() => {
		disposed = true;
		stopNotifications?.();
	});

	createEffect(() => {
		const dark = resolvedTheme() === "dark";
		document.documentElement.classList.toggle("dark", dark);
		document.documentElement.setAttribute("data-kb-theme", dark ? "dark" : "light");
	});

	createEffect(() => {
		const pendingRequest = incomingRequest.current;
		const availableVersion = appUpdates.state.availableVersion;
		appUpdates.setIncomingRequestActive(Boolean(pendingRequest));
		if (!pendingRequest && availableVersion) appUpdates.presentDeferredUpdate();
	});

	function handleWindowDrag(e: MouseEvent) {
		if (e.button !== 0 || e.detail > 1) return;
		if (isInteractiveTarget(e.target as HTMLElement)) return;
		getCurrentWindow()
			.startDragging()
			.catch(() => {});
	}

	async function handleChangeDestination() {
		const selected = await pickDestinationDirectory(currentPlatform);
		if (selected) await setDestinationDir(selected);
	}

	async function handleChangeDisplayName(displayName: string) {
		const updated = await setDeviceDisplayName(displayName);
		updateDisplayName(updated.display_name);
	}

	const addInternetPeerButton = () => (
		<Button
			variant="ghost"
			size="icon"
			class="h-6 w-6 text-muted-foreground"
			onClick={() =>
				navigate({
					to: "/",
					search: { internet: showInternetPeer() ? undefined : true, settings: undefined },
				})
			}
			title={t("addInternetPeer")}
			aria-label={t("addInternetPeer")}
		>
			<LucideCirclePlus width={16} height={16} />
		</Button>
	);

	return (
		<div class="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
			<header
				class="app-drag-region relative flex h-[calc(45px+env(safe-area-inset-top))] shrink-0 items-center gap-1.5 border-b border-border bg-muted pt-[env(safe-area-inset-top)]"
				classList={{
					"justify-start pl-3": isWindows,
					"justify-end pr-3 pl-[78px]": isMac,
					"justify-end px-3": !isWindows && !isMac,
				}}
				onMouseDown={handleWindowDrag}
			>
				<p
					class="pointer-events-none absolute truncate text-center text-xs font-medium text-muted-foreground"
					classList={{
						"inset-x-[140px]": !isMobile,
						"left-3 right-12": isMobile,
					}}
				>
					{device()
						? `${device()?.display_name} · ${formatDeviceHostname(device()?.hostname ?? "")}`
						: "Dukto"}
				</p>
				{isMac && addInternetPeerButton()}
				<Button
					variant="ghost"
					size="icon"
					class="h-6 w-6 text-muted-foreground"
					onClick={() =>
						navigate({
							to: "/",
							search: { internet: undefined, settings: showSettings() ? undefined : true },
						})
					}
					title={t("settings")}
					aria-label={t("settings")}
				>
					<LucideSettings width={16} height={16} />
				</Button>
				{!isMac && addInternetPeerButton()}
				<Show when={isWindows}>
					<WindowControls />
				</Show>
			</header>
			<div class="flex min-h-0 w-full flex-1 flex-col overflow-y-auto overflow-x-hidden px-4 pt-6 pb-[calc(1.5rem+env(safe-area-inset-bottom))]">
				<Outlet />
			</div>
			<SettingsModal
				open={showSettings()}
				currentPlatform={currentPlatform}
				displayName={settings()?.display_name ?? device()?.display_name ?? ""}
				destinationDir={settings()?.destination_dir ?? "..."}
				theme={theme()}
				language={language()}
				currentVersion={appUpdates.state.currentVersion}
				availableVersion={appUpdates.state.availableVersion}
				updateBusy={appUpdates.isBusy()}
				updateErrorCode={appUpdates.state.errorCode}
				updateErrorMessage={appUpdates.state.errorMessage}
				updateStatus={appUpdates.state.status}
				showAppUpdates={!isMobile}
				onChangeDestination={handleChangeDestination}
				onChangeDisplayName={handleChangeDisplayName}
				onChangeTheme={setTheme}
				onChangeLanguage={changeLanguage}
				onCheckForUpdates={() => appUpdates.checkForUpdates(false)}
				onClose={() => navigate({ to: "/", search: { internet: undefined, settings: undefined } })}
			/>
			<UpdateAvailableDialog />
		</div>
	);
}

export const Route = createRootRoute({
	component: RootLayout,
});
