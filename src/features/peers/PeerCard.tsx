import Icon from "../../components/Icon";

const platformIcons: Record<string, string> = {
	macos: "ic:baseline-apple",
	windows: "mdi:microsoft-windows",
	linux: "cib:linux",
};

export interface PeerCardProps {
	peer: {
		device_id: string;
		display_name: string;
		hostname: string;
		platform: string;
	};
	status?: string;
	onClick?: () => void;
}

function PeerCard(props: PeerCardProps) {
	const iconName = () => platformIcons[props.peer.platform] ?? "mdi:monitor";

	return (
		<button
			type="button"
			class="flex w-full items-center gap-3 rounded-lg border border-zinc-200 p-3 text-left transition-colors hover:bg-zinc-50 dark:border-zinc-800 dark:hover:bg-zinc-800/50"
			onClick={props.onClick}
		>
			<span class="flex h-9 w-9 items-center justify-center rounded-full bg-zinc-100 text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400">
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
