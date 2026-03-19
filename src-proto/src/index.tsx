import { For, createSignal } from "solid-js";
import { render } from "solid-js/web";
import Icon from "~/components/icon";
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
