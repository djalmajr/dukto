import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { type Accessor, For, createSignal, onMount } from "solid-js";
import { render } from "solid-js/web";
import PeerCard from "../../src/routes/-components/peers/peer-card";
import DropZone from "../../src/routes/-components/transfers/drop-zone";
import "../../src/styles/index.css";

// Exercise the real DropZone listener with the logical coordinates delivered
// by WKWebView, including a Retina screen, reorder, scroll and final drop.
mockIPC(() => undefined, { shouldMockEvents: true });
mockWindows("main");
Object.defineProperty(window, "devicePixelRatio", { configurable: true, value: 2 });
Object.defineProperty(navigator, "platform", { configurable: true, value: "MacIntel" });

function Regression() {
	const [ids, setIds] = createSignal(["linux", "windows", "third", "fourth"]);
	const [result, setResult] = createSignal("Running Retina file-drop regression…");
	let target: Accessor<string | undefined> = () => undefined;
	let received: { id: string; paths: string[] } | undefined;
	let viewport!: HTMLDivElement;
	const paths = ["/synthetic/sub"];
	function position(id: string) {
		const rect = document.querySelector(`[data-peer-id="${id}"]`)?.getBoundingClientRect();
		if (!rect) throw new Error(`Missing ${id}`);
		return { x: rect.left + 200, y: rect.top + rect.height / 2 };
	}
	onMount(() => {
		setTimeout(async () => {
			try {
				await emit("tauri://drag-enter", { paths, position: position("windows") });
				if (target() !== "windows") throw new Error("Hover targets the wrong host on Retina");
				await emit("tauri://drag-drop", { paths, position: position("windows") });
				if (received?.id !== "windows" || received.paths[0] !== paths[0] || target())
					throw new Error("Drop recipient or cleared highlight is wrong");
				setIds(["windows", "linux", "third", "fourth"]);
				await emit("tauri://drag-over", { position: position("linux") });
				if (target() !== "linux") throw new Error("Reordered hosts use a stale target");
				viewport.scrollTop = 70;
				await emit("tauri://drag-over", { position: position("third") });
				if (target() !== "third") throw new Error("Scroll offsets the target");
				await emit("tauri://drag-over", { position: { x: 5, y: 5 } });
				if (target()) throw new Error("Outside position keeps the previous host");
				viewport.scrollTop = 0;
				await emit("tauri://drag-over", { position: position("linux") });
				setResult("PASS: Retina hover/drop, reordered hosts, scrolling and outside area");
			} catch (error) {
				setResult(`FAIL: ${String(error)}`);
			}
		}, 0);
	});
	return (
		<main class="mx-auto max-w-lg p-6">
			<h1>{result()}</h1>
			<div ref={viewport} class="mt-8 h-64 overflow-y-auto p-2">
				<DropZone
					onFilesDropped={(id, paths) => {
						received = { id, paths };
					}}
				>
					{(currentTarget) => {
						target = currentTarget;
						return (
							<div class="space-y-2">
								<For each={ids()}>
									{(id) => (
										<PeerCard
											peer={{
												device_id: id,
												display_name: id,
												hostname: `${id}.local`,
												platform: id,
											}}
											dropHighlight={currentTarget() === id}
										/>
									)}
								</For>
							</div>
						);
					}}
				</DropZone>
			</div>
		</main>
	);
}
const root = document.getElementById("root");
if (!root) throw new Error("Missing fixture root");
render(() => <Regression />, root);
