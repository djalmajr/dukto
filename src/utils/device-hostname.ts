export function formatDeviceHostname(hostname: string): string {
	return hostname.replace(/\.local\.?$/i, "");
}
