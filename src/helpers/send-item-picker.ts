import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { type FileMetadataInfo, resolveFileMetadata } from "~/helpers/tauri";

interface AndroidFileSelectionEvent {
	error: string | null;
	files: FileMetadataInfo[] | null;
}

function pickAndroidFiles(media: boolean): Promise<FileMetadataInfo[] | null> {
	return new Promise((resolve, reject) => {
		let unlisten: (() => void) | undefined;
		void listen<AndroidFileSelectionEvent>("files:selected", ({ payload }) => {
			unlisten?.();
			if (payload.error) reject(new Error(payload.error));
			else resolve(payload.files);
		})
			.then((stopListening) => {
				unlisten = stopListening;
				return invoke("open_send_file_picker", { media, multiple: true });
			})
			.catch((error) => {
				unlisten?.();
				reject(error);
			});
	});
}

export function supportsFolderSelection(currentPlatform: string): boolean {
	return currentPlatform !== "ios";
}

export function supportsMediaSelection(currentPlatform: string): boolean {
	return currentPlatform === "android" || currentPlatform === "ios";
}

export async function pickSendItems(
	currentPlatform: string,
	directory: boolean,
	media = false,
): Promise<FileMetadataInfo[] | null> {
	if (directory && !supportsFolderSelection(currentPlatform)) return null;
	if (currentPlatform === "android" && !directory) return pickAndroidFiles(media);

	const selected = await open({
		multiple: true,
		directory,
		...(currentPlatform === "ios"
			? {
					fileAccessMode: "copy" as const,
					pickerMode: media ? ("media" as const) : ("document" as const),
				}
			: {}),
	});
	if (!selected) return null;
	const paths = Array.isArray(selected) ? selected : [selected];
	return resolveFileMetadata(paths);
}
