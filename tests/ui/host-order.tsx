import { createSignal, onCleanup, onMount } from "solid-js";
import { render } from "solid-js/web";
import HostOrder from "../../src/routes/-components/peers/host-order";
import PeerCard from "../../src/routes/-components/peers/peer-card";
import { movePeer } from "../../src/routes/-helpers/peer-order";
import "../../src/styles/index.css";

function Regression() {
	const [ids, setIds] = createSignal(["Linux", "Mac", "Windows"]);
	const [percent, setPercent] = createSignal(0);
	const [result, setResult] = createSignal(
		"Drag Linux below Windows. Transfers update every 50 ms.",
	);
	const originalControls = new Map<string, Element | null>();
	onMount(() => {
		for (const id of ids()) {
			originalControls.set(
				id,
				document.querySelector(`[data-peer-id="${id}"] button[data-transfer-abort]`),
			);
		}
	});
	const timer = window.setInterval(() => setPercent((value) => (value + 1) % 100), 50);
	onCleanup(() => clearInterval(timer));
	return (
		<main class="mx-auto max-w-lg p-6">
			<h1>{result()}</h1>
			<HostOrder
				ids={ids()}
				label={(id) => id}
				onMove={(source, target, after) => {
					const original = originalControls.get(source);
					setIds(movePeer(ids(), ids(), source, target, after));
					queueMicrotask(() => {
						const current = document.querySelector(
							`[data-peer-id="${source}"] button[data-transfer-abort]`,
						);
						setResult(
							original === current && original?.isConnected
								? `PASS: ${ids().join(", ")}; active transfer button preserved`
								: "FAIL: transfer remounted",
						);
					});
					return true;
				}}
			>
				{(id, header) => (
					<PeerCard
						peer={{
							device_id: id,
							display_name: id,
							hostname: `${id}.local`,
							platform: id.toLowerCase(),
						}}
						headerDrag={header}
						transfers={[
							{
								id: `${id}-transfer`,
								direction: "send",
								status: "active",
								percent: percent(),
								bytesSent: percent(),
								bytesTotal: 100,
							},
						]}
						onAbortTransfer={(transfer) =>
							setResult(
								transfer === `${id}-transfer`
									? `PASS: cancelled ${transfer}`
									: "FAIL: wrong transfer",
							)
						}
					/>
				)}
			</HostOrder>
		</main>
	);
}
const root = document.getElementById("root");
if (!root) throw new Error("Missing fixture root");
render(() => <Regression />, root);
