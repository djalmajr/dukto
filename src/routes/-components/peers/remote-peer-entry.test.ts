import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const entrySource = readFileSync(new URL("./remote-peer-entry.tsx", import.meta.url), "utf8");
const statusSource = readFileSync(new URL("./remote-session-status.tsx", import.meta.url), "utf8");
const listSource = readFileSync(new URL("./peer-list.tsx", import.meta.url), "utf8");
const tauriSource = readFileSync(new URL("../../../helpers/tauri.ts", import.meta.url), "utf8");
const rootSource = readFileSync(new URL("../../__root.tsx", import.meta.url), "utf8");
const internetCommandSource = readFileSync(
	new URL("../../../../src-tauri/src/commands/internet.rs", import.meta.url),
	"utf8",
);
const internetRegistrySource = readFileSync(
	new URL("../../../../src-tauri/src/state/internet_session.rs", import.meta.url),
	"utf8",
);

describe("remote peer entry contract", () => {
	test("keeps internet actions separate from LAN discovery", () => {
		expect(listSource).toContain("<RemotePeerEntry");
		expect(listSource.indexOf("<RemotePeerEntry")).toBeLessThan(
			listSource.indexOf("when={peerEntries().length > 0 || remotePeerConnected()}"),
		);
		expect(entrySource).toContain("createInternetInvite");
		expect(entrySource).toContain("importInternetInvite");
		expect(entrySource).not.toContain("peers[");
	});

	test("opens invitation actions from a platform-aware title bar control", () => {
		const titleBarClass = rootSource.match(/<header\s+class="([^"]+)"/)?.[1];

		expect(rootSource).toContain("~icons/lucide/circle-plus");
		expect(rootSource).toContain('t("addInternetPeer")');
		expect(titleBarClass).toContain("gap-1.5");
		expect(rootSource).not.toContain("bg-background/70");
		expect(rootSource.match(/hover:bg-muted-foreground\/10/g)?.length ?? 0).toBeGreaterThanOrEqual(
			2,
		);
		expect(rootSource.match(/variant="ghost"/g)?.length).toBeGreaterThanOrEqual(2);
		expect(rootSource).toContain("{isMac && addInternetPeerButton()}");
		expect(rootSource).toContain("{!isMac && addInternetPeerButton()}");
		expect(rootSource.indexOf("{isMac && addInternetPeerButton()}")).toBeLessThan(
			rootSource.indexOf("<LucideSettings width={16} height={16} />"),
		);
		expect(rootSource.indexOf("{!isMac && addInternetPeerButton()}")).toBeGreaterThan(
			rootSource.indexOf("<LucideSettings width={16} height={16} />"),
		);
	});

	test("merges an internet session with the authenticated application peer", () => {
		expect(tauriSource).toContain("peer: PeerIdentity | null");
		expect(entrySource).toContain("current.peer");
		expect(listSource).toContain("remotePeerId");
		expect(listSource).toContain("peer.device_id !== remotePeerId()");
		expect(entrySource).toContain("getTransfers");
	});

	test("keeps invitation controls in a modal and renders remote peers with the shared card", () => {
		expect(entrySource).toContain("<Dialog");
		expect(entrySource).toContain("<DialogContent");
		expect(entrySource).toContain("<DialogTitle");
		expect(entrySource).toContain("<Show when={visibleRemotePeer()}");
		expect(entrySource).toContain("<PeerCard");
		expect(entrySource.indexOf("<DialogContent")).toBeLessThan(
			entrySource.indexOf('t("createInternetInvitation")'),
		);
		expect(entrySource.indexOf("<Show when={visibleRemotePeer()}")).toBeLessThan(
			entrySource.indexOf("<SendPreview"),
		);
	});

	test("uses the same 80 percent window width as Settings", () => {
		// Mutation captured: restoring the peer-specific max-width makes the two feature dialogs diverge.
		const dialogClass = entrySource.match(/<DialogContent[\s\S]*?class="([^"]+)"/)?.[1];

		expect(dialogClass).toContain("w-4/5");
		expect(dialogClass).toContain("max-w-none");
	});

	test("treats a connected internet peer as a populated peer list", () => {
		expect(listSource).toContain("onConnectedChange={setRemotePeerConnected}");
		expect(listSource).toContain("peerEntries().length > 0 || remotePeerConnected()");
	});

	test("shows an immediate peer-card connection state instead of the empty list", () => {
		expect(entrySource).toContain("const [connecting, setConnecting]");
		expect(entrySource).toContain("isConnected() || connecting()");
		expect(entrySource).toContain('status={{ state: "connecting" }}');
		expect(entrySource).toContain('t("internetPeerConnecting")');
	});

	test("exposes named, keyboard-native create and connect controls", () => {
		expect(entrySource).toContain('type="button"');
		expect(entrySource).toContain('t("createInternetInvitation")');
		expect(entrySource).toContain('t("connectWithInvitation")');
		expect(statusSource).toContain('aria-live="polite"');
	});

	test("maps UI actions to the redacted Tauri command surface", () => {
		expect(tauriSource).toContain('invoke<InternetInviteShareView>("create_internet_invite")');
		expect(tauriSource).toContain('invoke<string>("get_internet_invitation_link"');
		expect(tauriSource).toContain('invoke<InternetInviteView>("import_internet_invite"');
		expect(tauriSource).toContain('invoke<string>("send_to_internet_session"');
		expect(tauriSource).toContain('invoke<void>("disconnect_internet_session"');
	});

	test("offers sending to both authenticated session peers after the remote session is ready", () => {
		expect(entrySource).toContain('status.state === "ready"');
		expect(entrySource).toContain("session()?.can_send");
		expect(entrySource).toContain("!current.can_send");
		expect(tauriSource).toContain("can_send: boolean");
		expect(internetRegistrySource).toContain("can_send: true");
		expect(internetCommandSource).not.toContain("internet invitation owner is receive-only");
		expect(internetCommandSource.match(/start_ready_internet_session\(/g)?.length ?? 0).toBe(3);
		expect(entrySource).toContain("pickSendItems");
		expect(entrySource).toContain("supportsFolderSelection");
		expect(entrySource).toContain("sendToInternetSession");
		expect(entrySource).toContain("<SendPreview");
		expect(entrySource).toContain("startSendTransfer");
	});

	test("keeps ready peers visible and removes them only after a terminal connection update", () => {
		// Mutation captured: rendering completed as terminal used to remove the peer after one send.
		expect(entrySource).toContain('listen<InternetInviteView>("internet:session-updated"');
		expect(entrySource).toContain("applySession(payload)");
		expect(entrySource).toContain('["failed", "cancelled", "expired"]');
		expect(entrySource).toContain('t("disconnectInternetSession")');
		expect(entrySource).toContain("pending() || session()");
		expect(entrySource).toContain("disabled={pending() || Boolean(session())}");
		expect(entrySource).toContain("setShare(null)");
		expect(entrySource).not.toContain("localStorage");
	});
});

describe("ephemeral invitation sharing contract", () => {
	test("renders backend-generated QR pixels and never injects invitation markup", () => {
		expect(entrySource).toContain("qr_svg_data_url");
		expect(entrySource).toContain("<img");
		expect(entrySource).not.toContain("innerHTML");
		expect(entrySource).not.toContain("localStorage");
		expect(entrySource).not.toContain("history.");
	});

	test("shows the invitation link, protects the pairing code, and clears both", () => {
		expect(entrySource).toMatch(/remoteInvitationLabel[\s\S]*?<TextFieldInput\s+type="text"/);
		expect(entrySource).toContain("readOnly={Boolean(session())}");
		expect(entrySource).toMatch(/pairingCodeLabel[\s\S]*?<TextFieldInput\s+type="password"/);
		expect(entrySource).toContain('autocomplete="off"');
		expect(entrySource).toContain('setInvitation("")');
		expect(entrySource).toContain("manual_code");
		expect(tauriSource).toContain('invoke<InternetInviteView>("confirm_internet_pairing_code"');
		expect(entrySource).toContain('inputmode="numeric"');
	});

	test("consumes registered deep links without logging or storing their bearer value", () => {
		expect(entrySource).toContain("getCurrent");
		expect(entrySource).toContain("onOpenUrl");
		expect(entrySource).not.toContain("console.");
		expect(entrySource).not.toContain("sessionStorage");
	});

	test("renders the owner invitation link and copies that explicit field value", () => {
		expect(entrySource).toContain(
			"setInvitation(await getInternetInvitationLink(created.session_id))",
		);
		expect(entrySource).toContain("navigator.clipboard.writeText(invitation())");
		expect(entrySource).toContain('t("invitationLinkCopied")');
	});
});
