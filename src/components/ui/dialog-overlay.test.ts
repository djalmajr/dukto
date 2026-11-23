import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const dialogSource = readFileSync(new URL("./dialog.tsx", import.meta.url), "utf8");
const settingsSource = readFileSync(
	new URL("../../routes/-components/settings/settings-modal.tsx", import.meta.url),
	"utf8",
);
const peerSource = readFileSync(
	new URL("../../routes/-components/peers/remote-peer-entry.tsx", import.meta.url),
	"utf8",
);
const updateSource = readFileSync(
	new URL("../../routes/-components/settings/update-available-dialog.tsx", import.meta.url),
	"utf8",
);
const incomingSource = readFileSync(
	new URL("../../routes/-components/transfers/incoming-request.tsx", import.meta.url),
	"utf8",
);

test("Settings and peer dialogs share the same dark overlay", () => {
	// Mutation captured: returning the shared overlay to bg-background/60 makes peer dialogs light again.
	expect(dialogSource).toContain("bg-black/40 backdrop-blur-[1px]");
	expect(dialogSource).not.toContain("overlayClass");
	expect(settingsSource).not.toContain("overlayClass");
	expect(peerSource).not.toContain("overlayClass");
});

test("every dialog uses the shared radius and content padding", () => {
	// Mutation captured: a breakpoint-only radius or per-dialog padding makes the modal surfaces diverge.
	expect(dialogSource).toContain("rounded-lg border bg-background p-5");
	expect(dialogSource).not.toContain("sm:rounded-lg");
	for (const source of [settingsSource, peerSource, updateSource, incomingSource]) {
		const dialogClass = source.match(/<DialogContent[\s\S]*?class="([^"]+)"/)?.[1];
		expect(dialogClass).toBeDefined();
		expect(dialogClass).not.toMatch(/(?:^|\s)(?:[a-z]+:)?(?:rounded(?:-[\w-]+)?|p-\d+)(?:\s|$)/);
	}
});
