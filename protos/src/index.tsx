import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import WindowFrame from "./components/WindowFrame";
import AppContent from "./screens/AppContent";
import "./styles.css";

function App() {
	const [showSettings, setShowSettings] = createSignal(false);

	return (
		<WindowFrame
			deviceName="djalmajr"
			hostname="MacBook-Pro.local"
			onSettingsClick={() => setShowSettings(!showSettings())}
		>
			<AppContent showSettings={showSettings()} onCloseSettings={() => setShowSettings(false)} />
		</WindowFrame>
	);
}

const root = document.getElementById("root");
if (!root) throw new Error("Root not found");
render(() => <App />, root);
