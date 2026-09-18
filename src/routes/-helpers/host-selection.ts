import { createSignal } from "solid-js";
import type { FileMetadataInfo } from "~/helpers/tauri";
import { mergeFiles } from "./selection";

export function createHostSelection(resolve: (paths: string[]) => Promise<FileMetadataInfo[]>) {
	const [drafts, setDrafts] = createSignal<Record<string, FileMetadataInfo[]>>({});
	const revisions = new Map<string, number>();

	function addResolved(hostId: string, files: FileMetadataInfo[], revision: number) {
		if (revision !== revisions.get(hostId) || files.length === 0) return files;
		setDrafts((current) => ({ ...current, [hostId]: mergeFiles(current[hostId] ?? [], files) }));
		return [];
	}

	function cancel(hostId: string) {
		const removed = drafts()[hostId] ?? [];
		revisions.set(hostId, (revisions.get(hostId) ?? 0) + 1);
		setDrafts((current) => {
			const next = { ...current };
			delete next[hostId];
			return next;
		});
		return removed;
	}

	async function add(hostId: string, paths: string[], revision = revisions.get(hostId) ?? 0) {
		revisions.set(hostId, revisions.get(hostId) ?? 0);
		const files = await resolve(paths);
		return addResolved(hostId, files, revision);
	}

	return {
		add,
		addResolved: (
			hostId: string,
			files: FileMetadataInfo[],
			revision = revisions.get(hostId) ?? 0,
		) => {
			revisions.set(hostId, revisions.get(hostId) ?? 0);
			return addResolved(hostId, files, revision);
		},
		cancel,
		files: (hostId: string) => drafts()[hostId] ?? [],
		revision: (hostId: string) => {
			const revision = revisions.get(hostId) ?? 0;
			revisions.set(hostId, revision);
			return revision;
		},
		remove: (hostId: string, path: string) => {
			const current = drafts()[hostId] ?? [];
			const removed = current.filter((file) => file.path === path);
			const remaining = current.filter((file) => file.path !== path);
			if (remaining.length === 0) cancel(hostId);
			else setDrafts((current) => ({ ...current, [hostId]: remaining }));
			return removed;
		},
		retainAvailable: (hostIds: string[]) => {
			const available = new Set(hostIds);
			const removed: FileMetadataInfo[] = [];
			for (const hostId of revisions.keys()) {
				if (!available.has(hostId)) removed.push(...cancel(hostId));
			}
			return removed;
		},
	};
}
