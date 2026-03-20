import { Outlet, createRootRoute, useNavigate, useSearch } from "@tanstack/solid-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createEffect } from "solid-js";
import LucideSettings from "~icons/lucide/settings";
import { changeLanguage, language } from "~/helpers/i18n";
import SettingsModal from "~/routes/-components/settings/settings-modal";
import { resolvedTheme, setDestinationDir, setTheme, settings, theme } from "~/stores/settings";

function isInteractiveTarget(target: HTMLElement) {
	return target.closest("button,[role='button'],a,input,select,textarea,[data-no-window-drag]");
}

function RootLayout() {
	const navigate = useNavigate();
	const search = useSearch({ strict: false });
	const showSettings = () => (search() as { settings?: boolean }).settings === true;

	createEffect(() => {
		const dark = resolvedTheme() === "dark";
		document.documentElement.classList.toggle("dark", dark);
		document.documentElement.setAttribute("data-kb-theme", dark ? "dark" : "light");
	});

	function handleWindowDrag(e: MouseEvent) {
		if (e.button !== 0 || e.detail > 1) return;
		if (isInteractiveTarget(e.target as HTMLElement)) return;
		getCurrentWindow().startDragging().catch(() => {});
	}

	async function handleChangeDestination() {
		try {
			const { open } = await import("@tauri-apps/plugin-dialog");
			const selected = await open({ directory: true, multiple: false });
			if (selected) {
				await setDestinationDir(selected as string);
			}
		} catch {
			/* not in Tauri */
		}
	}

	return (
		<div class="flex h-screen w-full flex-col overflow-hidden bg-background text-foreground">
			<header
				class="app-drag-region flex shrink-0 items-center justify-end px-2 py-1"
				style={{ "padding-left": "78px", "min-height": "28px" }}
				onMouseDown={handleWindowDrag}
			>
				<button
					type="button"
					class="flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
					onClick={() =>
						navigate({
							to: "/",
							search: { settings: showSettings() ? undefined : true },
						})
					}
					title="Settings"
				>
					<LucideSettings width={16} height={16} />
				</button>
			</header>
			<div class="flex w-full flex-1 flex-col items-center overflow-y-auto overflow-x-hidden px-4 pb-6">
				<Outlet />
			</div>
			<SettingsModal
				open={showSettings()}
				destinationDir={settings()?.destination_dir ?? "..."}
				theme={theme()}
				language={language()}
				onChangeDestination={handleChangeDestination}
				onChangeTheme={setTheme}
				onChangeLanguage={changeLanguage}
				onClose={() => navigate({ to: "/", search: { settings: undefined } })}
			/>
		</div>
	);
}

export const Route = createRootRoute({
	component: RootLayout,
});
