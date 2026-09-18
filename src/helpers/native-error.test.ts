import { expect, test } from "bun:test";
import { nativeErrorKey } from "./native-error";

test("native failures map to actionable translation keys", () => {
	for (const [error, key] of [
		["connection lost", "connectionLost"],
		["Connection reset by peer (os error 54)", "connectionLost"],
		["No space left on device (os error 28)", "diskFull"],
		["Transfer rejected by receiver", "transferRejected"],
		["Transfer rejected or expired.", "transferRejected"],
		["Permission denied (os error 13)", "permissionDenied"],
		["Peer abc not found", "peerUnavailable"],
		["Peer has no IPv4 address", "peerUnavailable"],
		["Received path traverses a symlink or escapes the selected destination", "invalidTransfer"],
		["Could not fetch a valid release JSON from the remote", "updateFeedUnavailable"],
	])
		expect(nativeErrorKey(error)).toBe(key);
});

test("unknown diagnostics never leak as untranslated UI text", () => {
	expect(nativeErrorKey("internal detail /private/path", "transferFailed")).toBe("transferFailed");
	expect(nativeErrorKey(null)).toBe("operationFailed");
});
