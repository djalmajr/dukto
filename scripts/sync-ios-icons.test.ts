import { afterEach, expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { syncIosIcons } from "./sync-ios-icons.mjs";

const temporaryDirectories: string[] = [];

afterEach(async () => {
	for (const directory of temporaryDirectories.splice(0)) {
		await rm(directory, { recursive: true, force: true });
	}
});

test("copies every catalog icon from the canonical iOS artwork", async () => {
	// Mutation captured: omitting a catalog filename leaves the generated Tauri artwork in the build.
	const directory = await mkdtemp(path.join(os.tmpdir(), "dukto-ios-icons-"));
	temporaryDirectories.push(directory);
	const source = path.join(directory, "source");
	const target = path.join(directory, "target");
	await mkdir(source);
	await mkdir(target);
	await writeFile(
		path.join(target, "Contents.json"),
		JSON.stringify({ images: [{ filename: "AppIcon-60x60@3x.png" }] }),
	);
	await writeFile(path.join(source, "AppIcon-60x60@3x.png"), "dukto-icon");
	await writeFile(path.join(target, "AppIcon-60x60@3x.png"), "tauri-icon");

	expect(await syncIosIcons(source, target)).toBe(true);
	expect(await readFile(path.join(target, "AppIcon-60x60@3x.png"), "utf8")).toBe("dukto-icon");
});
