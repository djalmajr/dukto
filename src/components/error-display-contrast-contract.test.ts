import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const tokens = readFileSync(new URL("../styles/solid-ui.css", import.meta.url), "utf8");

describe("error display dark theme contrast", () => {
	test("uses a dark error surface with a light foreground", () => {
		const darkTheme = tokens.match(/\/\* --- Dark theme --- \*\/([\s\S]*?)\/\* --- Register/)?.[1];
		expect(darkTheme).toContain("--error: 0 58% 28%;");
		expect(darkTheme).toContain("--error-foreground: 0 100% 88%;");
	});
});
