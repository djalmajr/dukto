import { createSignal } from "solid-js";
import type { PeerInfo } from "../../stores/peers";

const platformIcon: Record<string, string> = {
	macos: "🍎",
	windows: "🪟",
	linux: "🐧",
};

interface PeerCardProps {
	peer: PeerInfo;
	onSelect?: (peer: PeerInfo) => void;
}

function PeerCard(props: PeerCardProps) {
	const [isHover, setIsHover] = createSignal(false);
	const icon = () => platformIcon[props.peer.platform] ?? "💻";

	return (
		<button
			type="button"
			class="flex w-full items-center gap-3 rounded-lg border p-3 text-left transition-colors"
			classList={{
				"border-zinc-200 dark:border-zinc-800 hover:bg-zinc-50 dark:hover:bg-zinc-800/50":
					!isHover(),
				"border-blue-400 bg-blue-50 dark:border-blue-600 dark:bg-blue-950/30": isHover(),
			}}
			onMouseEnter={() => setIsHover(true)}
			onMouseLeave={() => setIsHover(false)}
			onClick={() => props.onSelect?.(props.peer)}
		>
			<span class="text-2xl">{icon()}</span>
			<div class="min-w-0 flex-1">
				<p class="truncate text-sm font-medium">{props.peer.display_name}</p>
				<p class="truncate text-xs text-zinc-500">{props.peer.hostname}</p>
			</div>
		</button>
	);
}

export default PeerCard;
