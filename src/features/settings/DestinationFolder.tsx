import { open } from "@tauri-apps/plugin-dialog";
import { Show } from "solid-js";
import { setDestinationDir, settings } from "../../stores/settings";

function DestinationFolder() {
	async function handlePick() {
		const selected = await open({ directory: true, multiple: false });
		if (selected) {
			await setDestinationDir(selected as string);
		}
	}

	return (
		<div class="space-y-1">
			<p class="text-xs font-medium text-zinc-500">Save files to</p>
			<div class="flex items-center gap-2">
				<Show when={settings()}>
					{(s) => (
						<span class="min-w-0 flex-1 truncate rounded-md border border-zinc-200 bg-zinc-100 px-2 py-1.5 text-xs dark:border-zinc-700 dark:bg-zinc-800">
							{s().destination_dir}
						</span>
					)}
				</Show>
				<button
					type="button"
					class="shrink-0 rounded-md border border-zinc-300 px-2 py-1.5 text-xs font-medium hover:bg-zinc-50 dark:border-zinc-700 dark:hover:bg-zinc-800"
					onClick={handlePick}
				>
					Change
				</button>
			</div>
		</div>
	);
}

export default DestinationFolder;
