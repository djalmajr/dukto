import { describe, expect, test } from "bun:test";
import { readFileSync, readdirSync } from "node:fs";
import { extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const sourceRoot = fileURLToPath(new URL("../", import.meta.url));

function sourceFiles(directory: string): string[] {
	return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
		const path = join(directory, entry.name);
		if (entry.isDirectory()) return sourceFiles(path);
		if (extname(entry.name) !== ".tsx" || entry.name.endsWith(".test.tsx")) return [];
		return [path];
	});
}

describe("Carbon icon contract", () => {
	test("uses Carbon for application controls while preserving platform brand icons", () => {
		// Mutation captured: reintroducing a non-Carbon control icon breaks the unified action language.
		const sources = sourceFiles(sourceRoot)
			.filter((path) => !path.endsWith("platform-icon.tsx"))
			.map((path) => readFileSync(path, "utf8"));
		const iconImports = sources.flatMap((source) =>
			[...source.matchAll(/from "(~icons\/[^"]+)"/g)].map((match) => match[1]),
		);

		expect(iconImports.length).toBeGreaterThan(0);
		expect(iconImports.every((icon) => icon.startsWith("~icons/carbon/"))).toBe(true);
		expect(iconImports).toContain("~icons/carbon/direct-link");
	});
});
