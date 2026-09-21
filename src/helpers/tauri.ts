import { invoke } from "@tauri-apps/api/core";
import type { PeerInfo } from "~/stores/peers";

export async function getPeers(): Promise<PeerInfo[]> {
	return invoke<PeerInfo[]>("get_peers");
}

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

export interface InternetInviteView {
	can_send: boolean;
	session_id: string;
	peer_id: string;
	expires_at_unix: number;
	status: { state: string; route?: "direct" | "relay" };
}

export interface InternetInviteShareView extends InternetInviteView {
	qr_svg_data_url: string;
	manual_code: string;
}

export async function createInternetInvite(): Promise<InternetInviteShareView> {
	return invoke<InternetInviteShareView>("create_internet_invite");
}

export async function getInternetInvitationLink(sessionId: string): Promise<string> {
	return invoke<string>("get_internet_invitation_link", { sessionId });
}

export async function importInternetInvite(invitation: string): Promise<InternetInviteView> {
	return invoke<InternetInviteView>("import_internet_invite", { invitation });
}

export async function cancelInternetInvite(sessionId: string): Promise<void> {
	return invoke<void>("cancel_internet_invite", { sessionId });
}

export async function disconnectInternetSession(sessionId: string): Promise<void> {
	return invoke<void>("disconnect_internet_session", { sessionId });
}

export async function confirmInternetPairingCode(
	sessionId: string,
	code: string,
): Promise<InternetInviteView> {
	return invoke<InternetInviteView>("confirm_internet_pairing_code", { sessionId, code });
}

export async function sendToInternetSession(sessionId: string, paths: string[]): Promise<string> {
	return invoke<string>("send_to_internet_session", { sessionId, paths });
}

export async function getDeviceInfo(): Promise<DeviceIdentity> {
	return invoke<DeviceIdentity>("get_device_info");
}

export async function resolveFileMetadata(paths: string[]): Promise<FileMetadataInfo[]> {
	return invoke<FileMetadataInfo[]>("resolve_file_metadata", { paths });
}

export async function releaseSelectedFiles(paths: string[]): Promise<void> {
	if (paths.length === 0) return;
	return invoke<void>("release_selected_files", { paths });
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

export async function cancelTransfer(transferId: string): Promise<void> {
	return invoke<void>("cancel_transfer", { transferId });
}
