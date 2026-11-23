import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const peerCardSource = readFileSync(new URL("./peer-card.tsx", import.meta.url), "utf8");
const peerListSource = readFileSync(new URL("./peer-list.tsx", import.meta.url), "utf8");
const hostOrderSource = readFileSync(new URL("./host-order.tsx", import.meta.url), "utf8");
const homePageSource = readFileSync(new URL("../../index.tsx", import.meta.url), "utf8");

describe("PeerCard hostname presentation contract", () => {
	test("formats the visible hostname instead of exposing the mDNS suffix", () => {
		expect(peerCardSource).toContain("{formatDeviceHostname(props.peer.hostname)}");
		expect(peerCardSource).not.toContain("{props.peer.hostname}");
	});

	test("formats the hostname used by the accessible actions label", () => {
		expect(peerCardSource).toMatch(/host:\s*formatDeviceHostname\(props\.peer\.hostname\)/);
		expect(peerCardSource).not.toMatch(/host:\s*props\.peer\.hostname/);
	});
});

describe("PeerCard unavailable action contract", () => {
	test("activates file actions by pointer or keyboard without forwarding the gesture to the host", () => {
		expect(peerCardSource).toContain("onSelect={activate}");
		expect(peerCardSource).toContain("onPointerDown={(event) => event.stopPropagation()}");
		expect(peerCardSource).toContain("onPointerUp={(event) => event.stopPropagation()}");
		expect(peerCardSource).toMatch(
			/onClick=\{\(event\) => \{\s*event\.stopPropagation\(\);\s*activate\(\);/,
		);
		expect(peerCardSource).toContain("onActivate={() => props.onAddFiles?.()}");
		expect(peerCardSource).toContain("onActivate={() => props.onAddFolders?.()}");
		expect(hostOrderSource).toContain(
			'document.querySelector(".peer-actions-menu[data-expanded]")',
		);
	});

	test("keeps the actions trigger mounted when actions are disabled", () => {
		expect(peerCardSource).toContain("when={hasActions() || props.actionsDisabled}");
		expect(peerCardSource).toContain("disabled={actionsUnavailable()}");
	});

	test("marks transient peers as unavailable instead of hiding their actions", () => {
		expect(peerListSource).toContain("actionsDisabled={props.actionsDisabled || !availablePeer()}");
	});

	test("offers the native media picker without replacing document actions", () => {
		expect(peerCardSource).toContain("when={props.onAddMedia}");
		expect(peerCardSource).toContain('{t("addPhotosAndVideos")}');
		expect(peerListSource).toContain("onAddMedia={addMedia()}");
		// Mutation captured: an iOS-only condition removes the Android media action.
		expect(homePageSource).toContain("supportsMediaSelection(currentPlatform)");
	});

	test("omits folder selection on iOS while preserving it on supported platforms", () => {
		// Mutation captured: wiring the folder callback unconditionally restores the crashing iOS action.
		expect(peerCardSource).toContain("when={props.onAddFolders}");
		expect(peerListSource).toContain("onAddFolders={addFolders()}");
		expect(homePageSource).toContain("supportsFolderSelection(currentPlatform)");
	});

	test("keeps connection metadata and disconnect in the host header actions", () => {
		// Mutation captured: moving disconnect back into expanded content leaves the existing popover incomplete.
		expect(peerCardSource).toContain("badge?: string");
		expect(peerCardSource).toContain("when={props.badge}");
		expect(peerCardSource).toContain("when={props.onDisconnect}");
		expect(peerCardSource).toContain('{t("disconnectInternetSession")}');
	});
});

describe("PeerCard expanded presentation contract", () => {
	test("renders every selected item with independent progress for both directions", () => {
		expect(peerCardSource).toContain("items?: TransferItemSlot[]");
		expect(peerCardSource).toContain("when={props.slot.items?.length}");
		expect(peerCardSource).toContain("item.item_id");
		expect(peerCardSource).toContain("item.name");
		expect(peerCardSource).toContain("item.bytes_sent");
		expect(peerCardSource).toContain("item.bytes_total");
		expect(peerCardSource).toContain("itemPercent(item)");
		expect(peerCardSource).toContain('role="progressbar"');
		expect(peerCardSource).toContain("max-h-52 space-y-2 overflow-y-auto");
		expect(peerCardSource).toContain("when={isAborting() && !props.slot.items?.length}");
		expect(peerCardSource).toContain(
			'when={!props.slot.items?.length && props.slot.direction === "receive" && isActive()}',
		);
		// Mutation captured: dropping the item list leaves only aggregate progress and hides selected entries.
		expect(peerCardSource).toContain("<For each={props.slot.items}>");
	});

	test("keeps the gray header square against expanded content", () => {
		expect(peerCardSource).toContain("const isExpanded = () =>");
		expect(peerCardSource).toContain('"rounded-b-xl": !isExpanded()');
		expect(peerCardSource).toContain("rounded-t-xl");
		expect(peerCardSource).not.toContain(
			'class="flex w-full items-center gap-3 px-3.5 py-3.5 rounded-xl',
		);
	});

	test("uses the high-contrast error token for inline transfer failures", () => {
		expect(peerCardSource).toContain('"text-error-foreground": isError()');
		expect(peerCardSource).toContain('class="text-xs text-error-foreground/90"');
	});
});
