import { getVersion } from "@tauri-apps/api/app";
import { Channel, Resource, invoke } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { type Update, check } from "@tauri-apps/plugin-updater";
import {
	type AppUpdateHandle,
	type UpdateDownloadEvent,
	createAppUpdateController,
} from "~/helpers/app-update-controller";

function wrapUpdate(update: Update): AppUpdateHandle {
	let bytesRid: number | null = null;
	let updateResourceOpen = true;
	const closeUpdateResource = async () => {
		if (!updateResourceOpen) return;
		await update.close();
		updateResourceOpen = false;
	};

	return {
		body: update.body,
		currentVersion: update.currentVersion,
		version: update.version,
		download: async (onEvent) => {
			const channel = new Channel<UpdateDownloadEvent>();
			channel.onmessage = onEvent;
			bytesRid = await invoke<number>("download_app_update", {
				updateRid: update.rid,
				onEvent: channel,
			});
			void closeUpdateResource().catch(() => {});
		},
		install: async () => {
			if (bytesRid === null) throw new Error("Download the update before installing it.");
			await invoke("install_app_update", { bytesRid });
			bytesRid = null;
		},
		close: async () => {
			if (bytesRid !== null) {
				await new Resource(bytesRid).close();
				bytesRid = null;
			}
			await closeUpdateResource();
		},
	};
}

export const appUpdates = createAppUpdateController({
	check: async () => {
		const update = await check({ timeout: 15_000 });
		return update ? wrapUpdate(update) : null;
	},
	getVersion,
	isDevelopment: import.meta.env.DEV,
	relaunch,
});
