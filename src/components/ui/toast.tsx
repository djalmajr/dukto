import type { JSX } from "solid-js";
import { Show } from "solid-js";
import { Portal } from "solid-js/web";

interface ToastProps {
	children: JSX.Element;
	open: boolean;
}

function Toast(props: ToastProps) {
	return (
		<Show when={props.open}>
			<Portal>
				<div class="pointer-events-none fixed inset-x-4 bottom-[calc(1rem+env(safe-area-inset-bottom))] z-[60] flex justify-center">
					<output
						class="max-w-sm rounded-md bg-foreground px-3 py-2 text-xs text-background shadow-lg"
						aria-live="polite"
					>
						{props.children}
					</output>
				</div>
			</Portal>
		</Show>
	);
}

export default Toast;
