import { Outlet, createRootRoute, useNavigate, useSearch } from "@tanstack/solid-router";
import { For, Show, createEffect, createSignal } from "solid-js";
import Icon from "~/components/icon";
import { changeLanguage, language } from "~/helpers/i18n";
import SettingsModal from "~/routes/-components/settings/settings-modal";
import IncomingRequestDialog from "~/routes/-components/transfers/incoming-request";
import {
	MOCK_FILES,
	type PeerInfo,
	clearAllTransfers,
	destinationDir,
	failTransfer,
	hidePeers,
	peers,
	resolvedTheme,
	screen,
	setScreen,
	setTheme,
	showPeers,
	startTransfer,
	theme,
} from "../stores/app";

function ProtoToolbar() {
	const scenarios = [
		{
			label: "No peers",
			run: () => {
				hidePeers();
				clearAllTransfers();
				setScreen({ id: "idle" });
			},
		},
		{
			label: "Show peers",
			run: () => {
				showPeers();
				clearAllTransfers();
				setScreen({ id: "idle" });
			},
		},
		{
			label: "Send to peer",
			run: () => {
				showPeers();
				clearAllTransfers();
				setScreen({ id: "preview", peer: peers[0], files: MOCK_FILES });
			},
		},
		{
			label: "Incoming",
			run: () => {
				showPeers();
				setScreen({
					id: "incoming",
					from: peers[0],
					itemCount: 4,
					totalSize: 12800000,
				});
			},
		},
		{
			label: "Send + Receive",
			run: () => {
				showPeers();
				clearAllTransfers();
				startTransfer(peers[0].device_id, "send", 237500000, "24.5 MB/s");
				startTransfer(peers[0].device_id, "receive", 12800000, "18.7 MB/s");
				setScreen({ id: "idle" });
			},
		},
		{
			label: "Error",
			run: () => {
				showPeers();
				clearAllTransfers();
				failTransfer(
					peers[0].device_id,
					"send",
					"Connection refused. The device may be offline or unreachable.",
				);
			},
		},
		{
			label: "Reset",
			run: () => {
				clearAllTransfers();
				setScreen({ id: "idle" });
			},
		},
	] as const;

	const [selectedScenario, setSelectedScenario] = createSignal(1);

	function applyScenario(index: number) {
		const clamped = Math.max(0, Math.min(index, scenarios.length - 1));
		setSelectedScenario(clamped);
		scenarios[clamped].run();
	}

	return (
		<div class="flex items-center gap-1">
			<button
				type="button"
				class="flex h-6 w-6 shrink-0 items-center justify-center rounded text-muted-foreground/60 transition-colors hover:text-foreground disabled:opacity-30"
				disabled={selectedScenario() === 0}
				onClick={() => applyScenario(selectedScenario() - 1)}
				title="Previous scenario"
			>
				<Icon name="lucide:chevron-left" size={14} />
			</button>
			<select
				class="h-6 rounded border border-border bg-background px-1.5 text-xs outline-none focus:border-primary"
				value={String(selectedScenario())}
				onChange={(event) => applyScenario(Number(event.currentTarget.value))}
			>
				<For each={scenarios}>
					{(scenario, index) => (
						<option value={String(index())} aria-label={scenario.label}>
							{scenario.label}
						</option>
					)}
				</For>
			</select>
			<button
				type="button"
				class="flex h-6 w-6 shrink-0 items-center justify-center rounded text-muted-foreground/60 transition-colors hover:text-foreground disabled:opacity-30"
				disabled={selectedScenario() === scenarios.length - 1}
				onClick={() => applyScenario(selectedScenario() + 1)}
				title="Next scenario"
			>
				<Icon name="lucide:chevron-right" size={14} />
			</button>
		</div>
	);
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

	return (
		<z-proto
			figma-key="U4hNHfRc8UGfcQk0GEEh8s"
			window-title="djalmajr · MacBook-Pro.local"
			window-width="480"
			window-height="640"
		>
			<z-proto-header>
				<ProtoToolbar />
			</z-proto-header>
			<z-proto-window-extras>
				<button
					type="button"
					class="flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
					onClick={() => navigate({ to: "/", search: showSettings() ? {} : { settings: true } })}
					title="Settings"
				>
					<Icon name="lucide:settings" size={16} />
				</button>
			</z-proto-window-extras>
			<z-proto-window-content>
				<div class="flex min-h-full w-full flex-1 flex-col items-center bg-background px-4 py-6 text-foreground">
					<Outlet />
				</div>
				<SettingsModal
					open={showSettings()}
					destinationDir={destinationDir()}
					theme={theme()}
					language={language()}
					onChangeDestination={() => {}}
					onChangeTheme={setTheme}
					onChangeLanguage={changeLanguage}
					onClose={() => navigate({ to: "/", search: {} })}
				/>
				<Show when={screen().id === "incoming" && screen()}>
					{(cur) => {
						const data = () => cur() as { from: PeerInfo; itemCount: number; totalSize: number };
						return (
							<IncomingRequestDialog
								request={{
									sender_name: data().from.display_name,
									item_count: data().itemCount,
									total_size: data().totalSize,
								}}
								onAccept={() => {
									startTransfer(data().from.device_id, "receive", data().totalSize, "18.7 MB/s");
									setScreen({ id: "idle" });
								}}
								onReject={() => setScreen({ id: "idle" })}
							/>
						);
					}}
				</Show>
			</z-proto-window-content>
		</z-proto>
	);
}

export const Route = createRootRoute({
	component: RootLayout,
});

declare module "solid-js" {
	namespace JSX {
		interface IntrinsicElements {
			"z-proto": JSX.HTMLAttributes<HTMLElement> & {
				"figma-key"?: string;
				"window-title"?: string;
				"window-width"?: string | number;
				"window-height"?: string | number;
				zoom?: string | number;
			};
			"z-proto-header": JSX.HTMLAttributes<HTMLElement>;
			"z-proto-window-extras": JSX.HTMLAttributes<HTMLElement>;
			"z-proto-window-content": JSX.HTMLAttributes<HTMLElement>;
		}
	}
}
