import { describe, expect, test } from "bun:test";
import { platformIcon } from "./platform";

describe("platformIcon", () => {
	test("returns apple icon for macos", () => {
		expect(platformIcon("macos")).toBe("ic:baseline-apple");
	});

	test("returns windows icon for windows", () => {
		expect(platformIcon("windows")).toBe("mdi:microsoft-windows");
	});

	test("returns linux icon for linux", () => {
		expect(platformIcon("linux")).toBe("cib:linux");
	});

	test("returns fallback icon for unknown platform", () => {
		expect(platformIcon("android")).toBe("mdi:monitor");
		expect(platformIcon("")).toBe("mdi:monitor");
	});
});
