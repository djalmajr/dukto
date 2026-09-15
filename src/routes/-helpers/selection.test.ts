import { expect, test } from "bun:test";
import { mergeFiles } from "./selection";

test("accumulates separate additions, keeps folders and distinguishes same names in different paths", () => {
	const file = { name: "a.txt", path: "/first/a.txt", size: 5, is_dir: false };
	const folder = { name: "folder", path: "/folder", size: 10, is_dir: true };
	const sameName = { ...file, path: "/second/a.txt" };
	const result = mergeFiles(mergeFiles(mergeFiles([], [file]), [folder]), [sameName, file]);
	expect(result).toEqual([file, folder, sameName]);
	expect(result.reduce((total, item) => total + item.size, 0)).toBe(20);
});
