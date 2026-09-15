import { For, Show } from "solid-js";
import PlatformIcon from "~/components/platform-icon";
import { Button } from "~/components/ui/button";
import { t } from "~/helpers/i18n";
import { formatBytes, sortFileItems } from "~/utils/format";
import LucideFile from "~icons/lucide/file";
import LucideFolder from "~icons/lucide/folder";
import LucideX from "~icons/lucide/x";

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
	const itemLabel = () => t("items", { count: sorted().length });

	return (
		<div
			class="overflow-hidden"
			classList={{
				"rounded-lg border border-border bg-card shadow-sm": !props.embedded,
			}}
		>
			{/* Header */}
			<Show when={!props.embedded}>
				<div class="flex items-center gap-3 border-b border-border bg-muted/50 px-4 py-3">
					<span class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-background text-muted-foreground shadow-sm">
						<PlatformIcon platform={props.peer.platform} class="h-5 w-5" />
					</span>
					<div class="min-w-0 flex-1">
						<p class="text-sm font-semibold leading-tight">
							{t("sendTo", { name: props.peer.display_name })}
						</p>
						<p class="text-xs text-muted-foreground">{props.peer.hostname}</p>
					</div>
					<Button
						variant="ghost"
						size="icon-sm"
						class="shrink-0 text-muted-foreground"
						aria-label={t("cancel")}
						onClick={props.onCancel}
					>
						<LucideX width={14} height={14} />
					</Button>
				</div>
			</Show>
			{/* Summary */}
			<div class="flex flex-wrap items-center justify-between gap-1 border-b border-border px-4 py-2">
				<p class="text-xs font-medium text-muted-foreground">
					{itemLabel()} &middot; {formatBytes(totalSize())}
				</p>
			</div>
			{/* File list */}
			<ul class="max-h-52 divide-y divide-border border-l border-border overflow-y-auto overflow-x-hidden">
				<For each={sorted()}>
					{(file) => (
						<li class="group flex items-center gap-2.5 px-4 py-2 transition-colors hover:bg-muted/50">
							<Show
								when={file.is_dir}
								fallback={<LucideFile class="h-4 w-4 shrink-0 text-muted-foreground" />}
							>
								<LucideFolder class="h-4 w-4 shrink-0 text-muted-foreground" />
							</Show>
							<span class="min-w-0 flex-1 truncate text-xs">{file.name}</span>
							<span class="shrink-0 text-xs tabular-nums text-muted-foreground">
								{formatBytes(file.size)}
							</span>
							{props.onRemoveFile && (
								<Button
									variant="ghost"
									size="icon-sm"
									class="shrink-0 text-muted-foreground/40 opacity-0 hover:text-foreground group-hover:opacity-100 focus-visible:opacity-100 focus-visible:text-foreground [&_svg]:size-3"
									onClick={() => props.onRemoveFile?.(file.path)}
									title={t("remove")}
									aria-label={t("removeFile", { name: file.name })}
								>
									<LucideX width={12} height={12} />
								</Button>
							)}
						</li>
					)}
				</For>
			</ul>
			{/* Actions */}
			<div class="flex gap-2 border-t border-border pt-3">
				<Button class="h-9 flex-1" onClick={props.onConfirm}>
					{t("send")}
				</Button>
				<Button variant="outline" class="h-9 flex-1" onClick={props.onCancel}>
					{t("cancel")}
				</Button>
			</div>
		</div>
	);
}

export default SendPreview;
