import { describe, expect, test } from "bun:test";
import { describeRemoteSession } from "./remote-session-status";

const NOW = 1_800_000_000;

describe("remote session status descriptions", () => {
	test("distinguishes direct and relay routes", () => {
		expect(describeRemoteSession({ state: "ready", route: "direct" }, NOW + 30, NOW).labelKey).toBe(
			"remoteStatusReadyDirect",
		);
		expect(
			describeRemoteSession({ state: "transferring", route: "relay" }, NOW + 30, NOW).labelKey,
		).toBe("remoteStatusTransferringRelay");
	});

	test("keeps an authenticated session active after its invitation expires", () => {
		// Mutation captured: applying the invitation deadline to ready sessions renders a live peer as expired.
		expect(describeRemoteSession({ state: "ready", route: "relay" }, NOW, NOW).labelKey).toBe(
			"remoteStatusReadyRelay",
		);
		expect(
			describeRemoteSession({ state: "transferring", route: "direct" }, NOW, NOW).labelKey,
		).toBe("remoteStatusTransferringDirect");
	});

	test("surfaces pairing, cancellation, expiry, and recoverable failure", () => {
		expect(describeRemoteSession({ state: "connecting" }, NOW + 30, NOW).labelKey).toBe(
			"remoteStatusConnecting",
		);
		expect(describeRemoteSession({ state: "pairing" }, NOW + 30, NOW).labelKey).toBe(
			"remoteStatusPairing",
		);
		expect(describeRemoteSession({ state: "cancelled" }, NOW + 30, NOW).tone).toBe("muted");
		expect(describeRemoteSession({ state: "failed" }, NOW + 30, NOW).tone).toBe("error");
		expect(describeRemoteSession({ state: "invited" }, NOW, NOW).labelKey).toBe(
			"remoteStatusExpired",
		);
	});
});
