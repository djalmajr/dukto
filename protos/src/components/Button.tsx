import type { JSX } from "solid-js";
import { resolvedTheme } from "../stores/app";

interface ButtonProps {
	children: JSX.Element;
	variant?: "primary" | "secondary";
	class?: string;
	onClick?: () => void;
}

function Button(props: ButtonProps) {
	const dark = () => resolvedTheme() === "dark";
	const primary = () => props.variant === "primary";

	return (
		<button
			type="button"
			class={`rounded-md px-3 py-2 text-[11px] font-medium transition-colors ${props.class ?? ""}`}
			classList={{
				"bg-blue-500 text-white hover:bg-blue-600": primary(),
				"border border-zinc-300 text-zinc-600 hover:bg-zinc-100": !primary() && !dark(),
				"border border-zinc-700 text-zinc-300 hover:bg-zinc-800": !primary() && dark(),
			}}
			onClick={props.onClick}
		>
			{props.children}
		</button>
	);
}

export default Button;
