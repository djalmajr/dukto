import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type Accessor, type JSX, createSignal, onCleanup, onMount } from "solid-js";
import { type FileMetadataInfo, resolveFileMetadata } from "~/helpers/tauri";

interface DropZoneProps {
	children: (isDragOver: Accessor<boolean>) => JSX.Element;
	onFilesDropped: (files: FileMetadataInfo[]) => void;
}

function DropZone(props: DropZoneProps) {
	const [isDragOver, setIsDragOver] = createSignal(false);
	let unlistenDrop: UnlistenFn | undefined;
	let unlistenHover: UnlistenFn | undefined;
	let unlistenLeave: UnlistenFn | undefined;

	onMount(async () => {
		const appWindow = getCurrentWebviewWindow();

		unlistenHover = await appWindow.onDragDropEvent((event) => {
			if (event.payload.type === "over") {
				setIsDragOver(true);
			} else if (event.payload.type === "drop") {
				setIsDragOver(false);
				const paths = event.payload.paths;
				if (paths.length > 0) {
					resolveFileMetadata(paths).then((files) => {
						props.onFilesDropped(files);
					});
				}
			} else if (event.payload.type === "leave") {
				setIsDragOver(false);
			}
		});
	});

	onCleanup(() => {
		unlistenDrop?.();
		unlistenHover?.();
		unlistenLeave?.();
	});

	return <>{props.children(isDragOver)}</>;
}

export default DropZone;
