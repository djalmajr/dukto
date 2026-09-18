// Native commands currently return diagnostic strings. Map known failures at
// the presentation boundary; keep the original error in state for diagnostics.
// Never display an unknown native string as translated user-facing text.
export function nativeErrorKey(
	error: string | null | undefined,
	fallback = "operationFailed",
): string {
	const message = (error ?? "").toLowerCase();
	if (/connection lost|connection reset|connection closed|broken pipe/.test(message))
		return "connectionLost";
	if (/timed out|timeout|deadline has elapsed/.test(message)) return "transferTimeout";
	if (/transfer rejected|rejected by receiver/.test(message)) return "transferRejected";
	if (/transfer cancel/.test(message)) return "transferCancelled";
	if (/no space left|disk full|not enough space/.test(message)) return "diskFull";
	if (/permission denied|access is denied|access denied/.test(message)) return "permissionDenied";
	if (/no such file|cannot find the file|cannot find the path/.test(message)) return "fileNotFound";
	if (
		/peer .*not found|peer has no ipv4 address|no route to host|connection refused|network is unreachable/.test(
			message,
		)
	)
		return "peerUnavailable";
	if (message.includes("incompatible protocol")) return "incompatibleProtocol";
	if (
		/does not match|unexpected packet|expected .*got|path traversal|absolute path rejected|binary chunk exceeds|received path traverses a symlink|escapes the selected destination/.test(
			message,
		)
	)
		return "invalidTransfer";
	if (/valid release json|release not found/.test(message)) return "updateFeedUnavailable";
	if (/signature/.test(message)) return "updateSignatureInvalid";
	if (message.includes("installing an update")) return "updateInProgress";
	return fallback;
}
