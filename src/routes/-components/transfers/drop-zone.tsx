import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type Accessor, type JSX, createSignal, onCleanup, onMount } from "solid-js";
import { fileDropPosition } from "~/routes/-helpers/file-drop-position";

interface DropZoneProps {
	children: (hostId: Accessor<string | undefined>) => JSX.Element;
	onFilesDropped: (hostId: string, paths: string[]) => void;
}

function DropZone(props: DropZoneProps) {
	const [targetId, setTargetId] = createSignal<string>();
	let unlisten: UnlistenFn | undefined;
	let disposed = false;

	onMount(async () => {
		const stop = await getCurrentWebviewWindow().onDragDropEvent(({ payload }) => {
			if (disposed) return;
			if (payload.type === "leave") {
				setTargetId(undefined);
				return;
			}
			const position = fileDropPosition(
				payload.position,
				navigator.platform,
				window.devicePixelRatio,
			);
			const hostId =
				document
					.elementFromPoint(position.x, position.y)
					?.closest("[data-peer-id]")
					?.getAttribute("data-peer-id") ?? undefined;
			if (payload.type === "drop") {
				setTargetId(undefined);
				if (hostId && payload.paths.length > 0) props.onFilesDropped(hostId, payload.paths);
			} else {
				setTargetId(hostId);
			}
		});
		if (disposed) stop();
		else unlisten = stop;
	});

	onCleanup(() => {
		disposed = true;
		unlisten?.();
	});

	return <>{props.children(targetId)}</>;
}

export default DropZone;
