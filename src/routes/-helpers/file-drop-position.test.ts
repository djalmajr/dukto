import { expect, test } from "bun:test";
import { fileDropPosition } from "./file-drop-position";

test("Retina Mac keeps the cursor over host 2 instead of halving its coordinates", () => {
	const position = fileDropPosition({ x: 370, y: 190 }, "MacIntel", 2);
	expect(position).toEqual({ x: 370, y: 190 });
	const hosts = [
		{ id: "host-1", top: 70, bottom: 138 },
		{ id: "host-2", top: 148, bottom: 218 },
	];
	expect(hosts.find((host) => position.y >= host.top && position.y < host.bottom)?.id).toBe(
		"host-2",
	);
});

test("Mac remains aligned when moved between Retina and non-Retina displays", () => {
	for (const ratio of [1, 2]) {
		expect(fileDropPosition({ x: 590, y: 220 }, "MacIntel", ratio)).toEqual({ x: 590, y: 220 });
	}
});

test("Windows physical pixels still convert to viewport coordinates", () => {
	for (const ratio of [1, 1.25, 1.5, 2]) {
		expect(fileDropPosition({ x: 400 * ratio, y: 190 * ratio }, "Win32", ratio)).toEqual({
			x: 400,
			y: 190,
		});
	}
});

test("Linux lab coordinates retain their existing conversion", () => {
	expect(fileDropPosition({ x: 400, y: 190 }, "Linux aarch64", 1)).toEqual({ x: 400, y: 190 });
});
