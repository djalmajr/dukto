import { For } from "solid-js";
import { formatBytes } from "../../lib/format";
import type { FileMetadataInfo } from "../../lib/tauri";

interface SendPreviewProps {
	files: FileMetadataInfo[];
	onConfirm: () => void;
	onCancel: () => void;
	onRemoveFile?: (path: string) => void;
}

function SendPreview(props: SendPreviewProps) {
	const totalSize = () => props.files.reduce((sum, f) => sum + f.size, 0);
	const itemLabel = () => (props.files.length === 1 ? "1 item" : `${props.files.length} items`);

	return (
		<div class="rounded-lg border border-zinc-200 bg-white p-4 shadow-sm dark:border-zinc-800 dark:bg-zinc-900">
			<h3 class="text-sm font-semibold">Send {itemLabel()}</h3>
			<p class="mt-1 text-xs text-zinc-500">{formatBytes(totalSize())} total</p>
			<ul class="mt-3 max-h-60 space-y-0.5 overflow-y-auto">
				<For each={props.files}>
					{(file) => (
						<li class="group flex items-center gap-2 rounded px-1 py-0.5 text-xs hover:bg-zinc-50 dark:hover:bg-zinc-800">
							<span class="shrink-0 text-zinc-400">{file.is_dir ? "\u{1F4C1}" : "\u{1F4C4}"}</span>
							<span class="min-w-0 flex-1 truncate">{file.name}</span>
							<span class="shrink-0 text-zinc-400">{formatBytes(file.size)}</span>
							{props.onRemoveFile && (
								<button
									type="button"
									class="ml-1 shrink-0 rounded p-0.5 text-zinc-300 opacity-0 transition-opacity hover:text-zinc-500 group-hover:opacity-100 dark:text-zinc-600 dark:hover:text-zinc-400"
									onClick={() => props.onRemoveFile?.(file.path)}
									title="Remove"
								>
									&#10005;
								</button>
							)}
						</li>
					)}
				</For>
			</ul>
			<div class="mt-4 flex gap-2">
				<button
					type="button"
					class="flex-1 rounded-md bg-blue-500 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-600"
					onClick={props.onConfirm}
				>
					Send
				</button>
				<button
					type="button"
					class="flex-1 rounded-md border border-zinc-300 px-3 py-1.5 text-xs font-medium text-zinc-700 hover:bg-zinc-50 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800"
					onClick={props.onCancel}
				>
					Cancel
				</button>
			</div>
		</div>
	);
}

export default SendPreview;
