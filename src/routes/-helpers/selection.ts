import type { FileMetadataInfo } from "~/helpers/tauri";

export function mergeFiles(existing: FileMetadataInfo[], added: FileMetadataInfo[]) {
	const files = new Map(existing.map((file) => [file.path, file]));
	for (const file of added) files.set(file.path, file);
	return [...files.values()];
}
