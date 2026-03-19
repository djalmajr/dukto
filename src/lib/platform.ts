export const platformIcons: Record<string, string> = {
	macos: "ic:baseline-apple",
	windows: "mdi:microsoft-windows",
	linux: "cib:linux",
};

export function platformIcon(platform: string): string {
	return platformIcons[platform] ?? "mdi:monitor";
}
