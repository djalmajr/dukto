import type { JSX } from "solid-js";

interface ModalProps {
	children: JSX.Element;
}

function Modal(props: ModalProps) {
	return (
		<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/30">
			<div class="w-full max-w-xs rounded-lg bg-white p-4 shadow-xl dark:bg-zinc-900">
				{props.children}
			</div>
		</div>
	);
}

export default Modal;
