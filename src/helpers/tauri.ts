import { invoke } from "@tauri-apps/api/core";

export interface DeviceIdentity {
	device_id: string;
	display_name: string;
	hostname: string;
	platform: string;
}

export interface FileMetadataInfo {
	name: string;
	path: string;
	size: number;
	is_dir: boolean;
}

export async function getDeviceInfo(): Promise<DeviceIdentity> {
	return invoke<DeviceIdentity>("get_device_info");
}

export async function resolveFileMetadata(paths: string[]): Promise<FileMetadataInfo[]> {
	return invoke<FileMetadataInfo[]>("resolve_file_metadata", { paths });
}

export async function sendToPeer(
	deviceId: string,
	paths: string[],
	peerAddress?: string,
	peerPort?: number,
): Promise<string> {
	return invoke<string>("send_to_peer", { deviceId, paths, peerAddress, peerPort });
}

export async function respondTransfer(transferId: string, accepted: boolean): Promise<void> {
	return invoke<void>("respond_transfer", { transferId, accepted });
}
