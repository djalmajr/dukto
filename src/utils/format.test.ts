import { describe, expect, test } from "bun:test";
import { formatBytes, sortFileItems } from "./format";

describe("formatBytes", () => {
	test("returns 0 B for zero", () => {
		expect(formatBytes(0)).toBe("0 B");
	});

	test("formats bytes", () => {
		expect(formatBytes(512)).toBe("512 B");
	});

	test("formats kilobytes", () => {
		expect(formatBytes(1024)).toBe("1.0 KB");
		expect(formatBytes(1536)).toBe("1.5 KB");
	});

	test("formats megabytes", () => {
		expect(formatBytes(1048576)).toBe("1.0 MB");
		expect(formatBytes(12800000)).toBe("12.2 MB");
	});

	test("formats gigabytes", () => {
		expect(formatBytes(1073741824)).toBe("1.0 GB");
	});
});

describe("sortFileItems", () => {
	test("directories come before files", () => {
		const items = [
			{ name: "readme.md", is_dir: false },
			{ name: "src", is_dir: true },
		];
		const sorted = sortFileItems(items);
		expect(sorted[0].name).toBe("src");
		expect(sorted[1].name).toBe("readme.md");
	});

	test("alphabetical within same type", () => {
		const items = [
			{ name: "zebra.txt", is_dir: false },
			{ name: "alpha.txt", is_dir: false },
			{ name: "docs", is_dir: true },
			{ name: "assets", is_dir: true },
		];
		const sorted = sortFileItems(items);
		expect(sorted.map((i) => i.name)).toEqual(["assets", "docs", "alpha.txt", "zebra.txt"]);
	});

	test("does not mutate original array", () => {
		const items = [
			{ name: "b.txt", is_dir: false },
			{ name: "a.txt", is_dir: false },
		];
		const sorted = sortFileItems(items);
		expect(items[0].name).toBe("b.txt");
		expect(sorted[0].name).toBe("a.txt");
	});

	test("empty array returns empty array", () => {
		expect(sortFileItems([])).toEqual([]);
	});
});
