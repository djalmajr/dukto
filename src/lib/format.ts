const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const i = Math.floor(Math.log(bytes) / Math.log(1024));
	const value = bytes / 1024 ** i;
	return `${value.toFixed(i > 0 ? 1 : 0)} ${UNITS[i]}`;
}
