import { createSignal } from "solid-js";
import type { FileMetadataInfo } from "~/helpers/tauri";
import { mergeFiles } from "./selection";

export function createHostSelection(resolve: (paths: string[]) => Promise<FileMetadataInfo[]>) {
	const [drafts, setDrafts] = createSignal<Record<string, FileMetadataInfo[]>>({});
	const revisions = new Map<string, number>();

	function cancel(hostId: string) {
		revisions.set(hostId, (revisions.get(hostId) ?? 0) + 1);
		setDrafts((current) => {
			const next = { ...current };
			delete next[hostId];
			return next;
		});
	}

	async function add(hostId: string, paths: string[], revision = revisions.get(hostId) ?? 0) {
		revisions.set(hostId, revisions.get(hostId) ?? 0);
		const files = await resolve(paths);
		if (revision !== revisions.get(hostId) || files.length === 0) return;
		setDrafts((current) => ({ ...current, [hostId]: mergeFiles(current[hostId] ?? [], files) }));
	}

	return {
		add,
		cancel,
		files: (hostId: string) => drafts()[hostId] ?? [],
		revision: (hostId: string) => {
			const revision = revisions.get(hostId) ?? 0;
			revisions.set(hostId, revision);
			return revision;
		},
		remove: (hostId: string, path: string) => {
			const remaining = (drafts()[hostId] ?? []).filter((file) => file.path !== path);
			if (remaining.length === 0) cancel(hostId);
			else setDrafts((current) => ({ ...current, [hostId]: remaining }));
		},
		retainAvailable: (hostIds: string[]) => {
			const available = new Set(hostIds);
			for (const hostId of revisions.keys()) {
				if (!available.has(hostId)) cancel(hostId);
			}
		},
	};
}
