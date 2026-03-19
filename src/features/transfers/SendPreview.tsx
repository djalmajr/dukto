import { For } from "solid-js";
import Button from "../../components/Button";
import Icon from "../../components/Icon";
import { formatBytes, sortFileItems } from "../../lib/format";
import { platformIcon } from "../../lib/platform";

export interface FileItem {
	name: string;
	path: string;
	size: number;
	is_dir: boolean;
}

export interface SendPreviewPeer {
	display_name: string;
	hostname: string;
	platform: string;
}

interface SendPreviewProps {
	peer: SendPreviewPeer;
	files: FileItem[];
	onConfirm: () => void;
	onCancel: () => void;
	onRemoveFile?: (path: string) => void;
}

function SendPreview(props: SendPreviewProps) {
	const sorted = () => sortFileItems(props.files);
	const totalSize = () => sorted().reduce((sum, f) => sum + f.size, 0);
	const itemLabel = () => (sorted().length === 1 ? "1 item" : `${sorted().length} items`);

	return (
		<div class="rounded-lg border border-zinc-200 bg-white dark:border-zinc-800 dark:bg-zinc-900">
			{/* Peer header */}
			<div class="flex items-center gap-3 border-b border-zinc-100 px-4 py-3 dark:border-zinc-800">
				<span class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-100 text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400">
					<Icon name={platformIcon(props.peer.platform)} size={18} />
				</span>
				<div class="min-w-0 flex-1">
					<p class="text-sm font-medium">Send to {props.peer.display_name}</p>
					<p class="text-[11px] text-zinc-500">{props.peer.hostname}</p>
				</div>
				<button
					type="button"
					class="text-zinc-400 hover:text-zinc-600 dark:hover:text-zinc-300"
					onClick={props.onCancel}
				>
					<Icon name="mdi:close" size={16} />
				</button>
			</div>
			{/* File list */}
			<div class="px-4 py-3">
				<p class="text-xs text-zinc-500">
					{itemLabel()} &middot; {formatBytes(totalSize())}
				</p>
				<ul class="mt-2 max-h-60 space-y-0.5 overflow-y-auto">
					<For each={sorted()}>
						{(file) => (
							<li class="group flex items-center gap-2 rounded px-1 py-0.5 text-xs hover:bg-zinc-50 dark:hover:bg-zinc-800">
								<Icon
									name={file.is_dir ? "mdi:folder-outline" : "mdi:file-outline"}
									size={14}
									class="shrink-0 text-zinc-400"
								/>
								<span class="min-w-0 flex-1 truncate">{file.name}</span>
								<span class="shrink-0 text-zinc-400">{formatBytes(file.size)}</span>
								{props.onRemoveFile && (
									<button
										type="button"
										class="ml-1 shrink-0 rounded p-0.5 text-zinc-300 opacity-0 transition-opacity hover:text-zinc-500 group-hover:opacity-100 dark:text-zinc-600 dark:hover:text-zinc-400"
										onClick={() => props.onRemoveFile?.(file.path)}
										title="Remove"
									>
										<Icon name="mdi:close" size={12} />
									</button>
								)}
							</li>
						)}
					</For>
				</ul>
				<div class="mt-3 flex gap-2">
					<Button variant="primary" class="flex-1" onClick={props.onConfirm}>
						Send
					</Button>
					<Button class="flex-1" onClick={props.onCancel}>
						Cancel
					</Button>
				</div>
			</div>
		</div>
	);
}

export default SendPreview;
