import { For, createSignal } from "solid-js";
import { render } from "solid-js/web";
import SettingsModal from "~/features/settings/settings-modal";
import WindowFrame from "./components/window-frame";
import AppContent from "./screens/app-content";
import {
	MOCK_FILES,
	clearAllTransfers,
	destinationDir,
	failTransfer,
	hidePeers,
	peers,
	setScreen,
	setTheme,
	showPeers,
	startTransfer,
	theme,
} from "./stores/app";
import "./styles.css";

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
		<div class="flex items-center gap-1.5">
			<button
				type="button"
				class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-40"
				disabled={selectedScenario() === 0}
				onClick={() => applyScenario(selectedScenario() - 1)}
				title="Previous scenario"
			>
				<svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
					<path stroke-linecap="round" stroke-linejoin="round" d="m15 18-6-6 6-6" />
				</svg>
			</button>
			<select
				class="h-8 min-w-48 rounded-md border border-border bg-background px-2 text-sm outline-none focus:border-primary"
				value={String(selectedScenario())}
				onChange={(event) => applyScenario(Number(event.currentTarget.value))}
			>
				<For each={scenarios}>
					{(scenario, index) => <option value={String(index())} aria-label={scenario.label}>{scenario.label}</option>}
				</For>
			</select>
			<button
				type="button"
				class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-border text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground disabled:opacity-40"
				disabled={selectedScenario() === scenarios.length - 1}
				onClick={() => applyScenario(selectedScenario() + 1)}
				title="Next scenario"
			>
				<svg class="h-4 w-4" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true">
					<path stroke-linecap="round" stroke-linejoin="round" d="m9 18 6-6-6-6" />
				</svg>
			</button>
		</div>
	);
}

function App() {
	const [showSettings, setShowSettings] = createSignal(false);

	return (
		<WindowFrame
			deviceName="djalmajr"
			hostname="MacBook-Pro.local"
			onSettingsClick={() => setShowSettings(!showSettings())}
			overlay={
				<SettingsModal
					open={showSettings()}
					destinationDir={destinationDir()}
					theme={theme()}
					onChangeDestination={() => {}}
					onChangeTheme={setTheme}
					onClose={() => setShowSettings(false)}
				/>
			}
			toolbar={<ProtoToolbar />}
		>
			<AppContent />
		</WindowFrame>
	);
}

const root = document.getElementById("root");
if (!root) throw new Error("Root not found");
render(() => <App />, root);
