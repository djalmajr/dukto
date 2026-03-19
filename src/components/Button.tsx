import type { JSX } from "solid-js";

interface ButtonProps {
	children: JSX.Element;
	variant?: "primary" | "secondary";
	class?: string;
	onClick?: () => void;
}

function Button(props: ButtonProps) {
	const primary = () => props.variant === "primary";

	return (
		<button
			type="button"
			class={`rounded-md px-3 py-2 text-xs font-medium transition-colors ${props.class ?? ""}`}
			classList={{
				"bg-blue-500 text-white hover:bg-blue-600": primary(),
				"border border-zinc-300 text-zinc-600 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-300 dark:hover:bg-zinc-800":
					!primary(),
			}}
			onClick={props.onClick}
		>
			{props.children}
		</button>
	);
}

export default Button;
