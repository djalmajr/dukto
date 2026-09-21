import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const source = readFileSync(new URL("./settings-modal.tsx", import.meta.url), "utf8");
const toastSource = readFileSync(
	new URL("../../../components/ui/toast.tsx", import.meta.url),
	"utf8",
);

describe("settings modal organization", () => {
	test("uses 80 percent of the available window width", () => {
		// Mutation captured: restoring the compact max-width cap keeps Settings narrower than the requested proportion.
		const dialogClass = source.match(/<DialogContent[\s\S]*?class="([^"]+)"/)?.[1];

		expect(dialogClass).toContain("w-4/5");
		expect(dialogClass).toContain("max-w-none");
	});

	test("places the outlined update action below the language control and outside the footer", () => {
		// Mutation captured: moving the update action back into the footer makes update status compete with app metadata.
		const languageControl = source.indexOf('t("language")');
		const updateAction = source.indexOf('t("checkForUpdates")');
		const footer = source.indexOf("<footer");

		expect(languageControl).toBeGreaterThan(-1);
		expect(updateAction).toBeGreaterThan(languageControl);
		expect(updateAction).toBeLessThan(footer);
		expect(source.slice(languageControl, footer)).toContain('variant="outline"');
	});

	test("keeps version left and Privacy right in a metadata-only footer", () => {
		// Mutation captured: returning the separator or the update action to the footer breaks its lightweight metadata layout.
		const footer = source.slice(source.indexOf("<footer"), source.indexOf("</footer>") + 9);

		expect(footer).toContain("justify-between");
		expect(footer).not.toContain("border-t");
		expect(footer.indexOf("props.currentVersion")).toBeLessThan(
			footer.indexOf('t("privacyPolicy")'),
		);
		expect(footer).not.toContain('t("checkForUpdates")');
		expect(footer).not.toContain('t("upToDate")');
		expect(footer).toContain('<CarbonLaunch class="size-3.5" />');
	});

	test("shows the latest-version result as a transient toast only after a manual check", () => {
		// Mutation captured: rendering upToDate directly from updateStatus makes a startup check leave persistent copy.
		expect(source).toContain("await props.onCheckForUpdates()");
		expect(source).toContain('props.updateStatus === "upToDate"');
		expect(source).toContain("<Toast");
		expect(source).toContain("open={upToDateToastVisible()}");
		expect(toastSource).toContain("<output");
		expect(toastSource).toContain('aria-live="polite"');
		expect(source).toContain('t("upToDate")');
		expect(source).not.toContain('case "upToDate"');
	});
});
