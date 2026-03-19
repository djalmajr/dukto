import { For } from "solid-js";
import Icon from "~/components/icon";
import { Button } from "~/components/ui/button";
import { formatBytes, sortFileItems } from "~/lib/format";
import { platformIcon } from "~/lib/platform";

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
	embedded?: boolean;
	onConfirm: () => void;
	onCancel: () => void;
	onRemoveFile?: (path: string) => void;
}

function SendPreview(props: SendPreviewProps) {
	const sorted = () => sortFileItems(props.files);
	const totalSize = () => sorted().reduce((sum, f) => sum + f.size, 0);
	const itemLabel = () => (sorted().length === 1 ? "1 item" : `${sorted().length} items`);

	return (
		<div
			class="overflow-hidden"
			classList={{
				"rounded-lg border border-border bg-card shadow-sm": !props.embedded,
			}}
		>
			{/* Header */}
			{!props.embedded && (
				<div class="flex items-center gap-3 border-b border-border bg-muted/50 px-4 py-3">
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-background text-muted-foreground shadow-sm">
						<Icon name={platformIcon(props.peer.platform)} size={20} />
					</span>
					<div class="min-w-0 flex-1">
						<p class="text-sm font-semibold leading-tight">Send to {props.peer.display_name}</p>
						<p class="text-xs text-muted-foreground">{props.peer.hostname}</p>
					</div>
					<button
						type="button"
						class="flex h-6 w-6 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
						onClick={props.onCancel}
					>
						<Icon name="mdi:close" size={14} />
					</button>
				</div>
			)}
			{/* Summary */}
			<div class="border-b border-border px-4 py-2">
				<p class="text-xs font-medium text-muted-foreground">
					{itemLabel()} &middot; {formatBytes(totalSize())}
				</p>
			</div>
			{/* File list */}
			<ul class="max-h-52 divide-y divide-border border-l border-border overflow-y-auto">
				<For each={sorted()}>
					{(file) => (
						<li class="group flex items-center gap-2.5 px-4 py-2 transition-colors hover:bg-muted/50">
							<Icon
								name={file.is_dir ? "mdi:folder-outline" : "mdi:file-outline"}
								size={16}
								class="shrink-0 text-muted-foreground"
							/>
							<span class="min-w-0 flex-1 truncate text-xs">{file.name}</span>
							<span class="shrink-0 text-xs tabular-nums text-muted-foreground">
								{formatBytes(file.size)}
							</span>
							{props.onRemoveFile && (
								<button
									type="button"
									class="flex h-5 w-5 shrink-0 items-center justify-center rounded text-muted-foreground/40 opacity-0 transition-all hover:text-foreground group-hover:opacity-100"
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
			{/* Actions */}
			<div class="flex gap-2 border-t border-border pt-3">
				<Button size="sm" class="flex-1" onClick={props.onConfirm}>
					Send
				</Button>
				<Button variant="outline" size="sm" class="flex-1" onClick={props.onCancel}>
					Cancel
				</Button>
			</div>
		</div>
	);
}

export default SendPreview;
