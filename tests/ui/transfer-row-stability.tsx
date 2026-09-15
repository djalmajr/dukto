import { createSignal, onMount } from "solid-js";
import { render } from "solid-js/web";
import PeerCard, { type TransferSlot } from "../../src/routes/-components/peers/peer-card";

// Open /tests/ui/transfer-row-stability.html with the Vite development server.
// A progress update between pointer-down and pointer-up must not replace the button.
function Regression() {
	const [slots, setSlots] = createSignal<TransferSlot[]>([
		{
			id: "active-one",
			direction: "receive",
			status: "active",
			percent: 10,
			bytesSent: 10,
			bytesTotal: 100,
		},
	]);
	const [result, setResult] = createSignal("Running");
	onMount(() => {
		const original = document.querySelector<HTMLButtonElement>("button[data-transfer-abort]");
		setSlots(slots().map((slot) => ({ ...slot, percent: 50, bytesSent: 50 })));
		queueMicrotask(() => {
			const current = document.querySelector("button[data-transfer-abort]");
			if (!original || original !== current || !original.isConnected) {
				setResult("FAIL: progress update replaces cancel button");
				return;
			}
			original.click();
		});
	});
	return (
		<>
			<h1>{result()}</h1>
			<PeerCard
				peer={{ device_id: "peer", display_name: "Linux", hostname: "lab", platform: "linux" }}
				transfers={slots()}
				onAbortTransfer={(id) =>
					setResult(
						id === "active-one"
							? "PASS: stable button cancels the correct transfer after progress update"
							: "FAIL: wrong transfer cancelled",
					)
				}
			/>
		</>
	);
}

const root = document.getElementById("root");
if (!root) throw new Error("Missing regression root");
render(() => <Regression />, root);
