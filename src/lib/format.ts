const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const i = Math.floor(Math.log(bytes) / Math.log(1024));
	const value = bytes / 1024 ** i;
	return `${value.toFixed(i > 0 ? 1 : 0)} ${UNITS[i]}`;
}

/** Sort items: directories first (A-Z), then files (A-Z). */
export function sortFileItems<T extends { name: string; is_dir: boolean }>(items: T[]): T[] {
	return [...items].sort((a, b) => {
		if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1;
		return a.name.localeCompare(b.name);
	});
}
