import { describe, expect, test } from "bun:test";
import type { FileMetadataInfo } from "~/helpers/tauri";
import { createHostSelection } from "./host-selection";

const file = (path: string): FileMetadataInfo => ({
	path,
	name: path.split("/").pop() ?? path,
	size: 1,
	is_dir: false,
});

describe("host-bound selection", () => {
	test("adding, removing and canceling one recipient never changes the other", async () => {
		const selection = createHostSelection(async (paths) => paths.map(file));
		await selection.add("mac", ["/a.txt"]);
		await selection.add("windows", ["/b.txt"]);
		await selection.add("mac", ["/a.txt", "/c.txt"]);
		expect(selection.files("mac").map((f) => f.path)).toEqual(["/a.txt", "/c.txt"]);
		expect(selection.files("windows").map((f) => f.path)).toEqual(["/b.txt"]);
		selection.remove("mac", "/a.txt");
		selection.cancel("mac");
		expect(selection.files("mac")).toEqual([]);
		expect(selection.files("windows").map((f) => f.path)).toEqual(["/b.txt"]);
	});

	test("out-of-order picker results keep the initiating recipient", async () => {
		let finish: (files: FileMetadataInfo[]) => void = () => {};
		const selection = createHostSelection((paths) =>
			paths[0] === "/slow"
				? new Promise((resolve) => {
						finish = resolve;
					})
				: Promise.resolve(paths.map(file)),
		);
		const pending = selection.add("mac", ["/slow"]);
		await selection.add("windows", ["/fast"]);
		finish([file("/slow")]);
		await pending;
		expect(selection.files("mac").map((f) => f.path)).toEqual(["/slow"]);
		expect(selection.files("windows").map((f) => f.path)).toEqual(["/fast"]);
	});

	test("canceled or disconnected host cannot regain stale picker results after returning", async () => {
		const selection = createHostSelection(async (paths) => paths.map(file));
		const oldRevision = selection.revision("mac");
		await selection.add("windows", ["/keep"]);
		selection.retainAvailable(["windows"]);
		await selection.add("mac", ["/stale"], oldRevision);
		expect(selection.files("mac")).toEqual([]);
		await selection.add("mac", ["/fresh"]);
		const canceledRevision = selection.revision("mac");
		selection.cancel("mac");
		await selection.add("mac", ["/also-stale"], canceledRevision);
		expect(selection.files("mac")).toEqual([]);
		expect(selection.files("windows").map((f) => f.path)).toEqual(["/keep"]);
	});
});
