import { For, Show } from "solid-js";
import Button from "../../components/Button";
import Modal from "../../components/Modal";

export type ThemeMode = "light" | "dark" | "system";

interface SettingsModalProps {
	visible: boolean;
	destinationDir: string;
	theme: ThemeMode;
	onChangeDestination: () => void;
	onChangeTheme: (theme: ThemeMode) => void;
	onClose: () => void;
}

function SettingsModal(props: SettingsModalProps) {
	return (
		<Show when={props.visible}>
			<Modal>
				<h3 class="text-sm font-semibold">Settings</h3>
				<div class="mt-4 space-y-4">
					<div class="space-y-1">
						<p class="text-[11px] font-medium text-zinc-500">Save files to</p>
						<div class="flex items-center gap-2">
							<span class="min-w-0 flex-1 truncate rounded-md border border-zinc-200 bg-zinc-100 px-2 py-1.5 text-[11px] text-zinc-700 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-300">
								{props.destinationDir}
							</span>
							<Button onClick={props.onChangeDestination}>Change</Button>
						</div>
					</div>
					<div class="space-y-1">
						<p class="text-[11px] font-medium text-zinc-500">Appearance</p>
						<div class="flex gap-1">
							<For each={["light", "dark", "system"] as ThemeMode[]}>
								{(opt) => (
									<button
										class="flex-1 rounded-md border px-2 py-1.5 text-[11px] font-medium capitalize transition-colors"
										classList={{
											"border-blue-500 bg-blue-50 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400":
												props.theme === opt,
											"border-zinc-300 text-zinc-500 hover:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-400 dark:hover:bg-zinc-800":
												props.theme !== opt,
										}}
										onClick={() => props.onChangeTheme(opt)}
									>
										{opt}
									</button>
								)}
							</For>
						</div>
					</div>
				</div>
				<Button class="mt-5 w-full" onClick={props.onClose}>
					Done
				</Button>
			</Modal>
		</Show>
	);
}

export default SettingsModal;
