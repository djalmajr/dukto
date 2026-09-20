import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const entrySource = readFileSync(new URL("./remote-peer-entry.tsx", import.meta.url), "utf8");
const statusSource = readFileSync(new URL("./remote-session-status.tsx", import.meta.url), "utf8");
const listSource = readFileSync(new URL("./peer-list.tsx", import.meta.url), "utf8");
const tauriSource = readFileSync(new URL("../../../helpers/tauri.ts", import.meta.url), "utf8");

describe("remote peer entry contract", () => {
	test("keeps internet actions separate from LAN discovery", () => {
		expect(listSource).toContain("<RemotePeerEntry />");
		expect(listSource.indexOf("<RemotePeerEntry />")).toBeLessThan(
			listSource.indexOf("when={peerEntries().length > 0}"),
		);
		expect(entrySource).toContain("createInternetInvite");
		expect(entrySource).toContain("importInternetInvite");
		expect(entrySource).not.toContain("peers[");
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
	});

	test("offers sending only to the invitation joiner after the remote session is ready", () => {
		// Mutation captured: dropping can_send from the ready block re-exposes owner send actions.
		expect(entrySource).toContain('status.state === "ready"');
		expect(entrySource).toContain("session()?.can_send");
		expect(entrySource).toContain("!current.can_send");
		expect(tauriSource).toContain("can_send: boolean");
		expect(entrySource).toContain("pickSendItems");
		expect(entrySource).toContain("supportsFolderSelection");
		expect(entrySource).toContain("sendToInternetSession");
		expect(entrySource).toContain("<SendPreview");
		expect(entrySource).toContain("startSendTransfer");
	});

	test("projects backend session route and terminal updates without persisting invitations", () => {
		expect(entrySource).toContain('listen<InternetInviteView>("internet:session-updated"');
		expect(entrySource).toContain("setSession(payload)");
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

	test("keeps bearer input obscured and clears it after consumption", () => {
		expect(entrySource).toContain('type="password"');
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

	test("copies a bearer link only after an explicit user action and does not render it", () => {
		expect(entrySource).toContain("getInternetInvitationLink(current.session_id)");
		expect(entrySource).toContain("navigator.clipboard.writeText(invitationLink)");
		expect(entrySource).toContain('invitationLink = ""');
		expect(entrySource).not.toContain("setInvitationLink");
		expect(entrySource).toContain('t("invitationLinkCopied")');
	});
});
