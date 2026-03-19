import { resolvedTheme } from "../stores/app";
import type { PeerInfo } from "../stores/app";
import Icon from "./Icon";

const platformIcons: Record<string, string> = {
	macos: "ic:baseline-apple",
	windows: "mdi:microsoft-windows",
	linux: "cib:linux",
};

interface PeerCardProps {
	peer: PeerInfo;
	status?: string;
	onClick?: () => void;
}

function PeerCard(props: PeerCardProps) {
	const dark = () => resolvedTheme() === "dark";
	const iconName = () => platformIcons[props.peer.platform] ?? "mdi:monitor";

	return (
		<button
			type="button"
			class="flex w-full items-center gap-3 rounded-lg border p-3 text-left transition-colors"
			classList={{
				"border-zinc-200 hover:bg-zinc-100": !dark(),
				"border-zinc-800 hover:bg-zinc-800/50": dark(),
			}}
			onClick={props.onClick}
		>
			<span
				class="flex h-9 w-9 items-center justify-center rounded-full"
				classList={{
					"bg-zinc-100 text-zinc-500": !dark(),
					"bg-zinc-800 text-zinc-400": dark(),
				}}
			>
				<Icon name={iconName()} size={20} />
			</span>
			<div class="min-w-0 flex-1">
				<p class="truncate text-sm font-medium">{props.peer.display_name}</p>
				<p class="truncate text-xs text-zinc-500">{props.peer.hostname}</p>
			</div>
			{props.status && <span class="shrink-0 text-[11px] text-blue-500">{props.status}</span>}
		</button>
	);
}

export default PeerCard;
