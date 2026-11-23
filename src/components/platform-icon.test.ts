import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const source = readFileSync(fileURLToPath(new URL("./platform-icon.tsx", import.meta.url)), "utf8");

describe("peer platform icons", () => {
	test("shows Android and iPhone icons instead of the desktop fallback", () => {
		// Mutation captured: removing either mobile platform match restores the monitor icon on that device.
		const android = source.match(/<Match when=\{props\.platform === "android"\}>\s*<([\w]+) /);
		const ios = source.match(/<Match when=\{props\.platform === "ios"\}>\s*<([\w]+) /);

		expect(android?.[1]).toBe("MdiAndroid");
		expect(ios?.[1]).toBe("MdiCellphoneIphone");
		expect(source).toContain('import MdiAndroid from "~icons/mdi/android"');
		expect(source).toContain('import MdiCellphoneIphone from "~icons/mdi/cellphone-iphone"');
	});
});
