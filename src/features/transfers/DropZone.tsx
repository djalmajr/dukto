import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { type JSX, createSignal, onCleanup, onMount } from "solid-js";
import { type FileMetadataInfo, resolveFileMetadata } from "../../lib/tauri";

interface DropZoneProps {
	children: JSX.Element;
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

	return (
		<div
			class="relative min-h-screen"
			classList={{
				"ring-2 ring-blue-500 ring-inset bg-blue-50/50 dark:bg-blue-950/20": isDragOver(),
			}}
		>
			{props.children}
			{isDragOver() && (
				<div class="pointer-events-none absolute inset-0 flex items-center justify-center">
					<div class="rounded-lg bg-blue-500 px-6 py-3 text-sm font-medium text-white shadow-lg">
						Drop files to send
					</div>
				</div>
			)}
		</div>
	);
}

export default DropZone;
