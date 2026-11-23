import { copyFile, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourceDirectory = path.join(root, "src-tauri", "icons", "ios");
const targetDirectory = path.join(
	root,
	"src-tauri",
	"gen",
	"apple",
	"Assets.xcassets",
	"AppIcon.appiconset",
);

export async function syncIosIcons(source = sourceDirectory, target = targetDirectory) {
	let manifest;
	try {
		manifest = JSON.parse(await readFile(path.join(target, "Contents.json"), "utf8"));
	} catch (error) {
		if (error.code === "ENOENT") return false;
		throw error;
	}

	const filenames = new Set(manifest.images.map((image) => image.filename));
	for (const filename of filenames) {
		if (!filename || path.basename(filename) !== filename) {
			throw new Error(`Invalid iOS icon filename: ${filename}`);
		}
		await stat(path.join(source, filename));
		await copyFile(path.join(source, filename), path.join(target, filename));
	}
	return true;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	const synced = await syncIosIcons();
	if (synced) console.log("Synchronized iOS app icons from src-tauri/icons/ios");
}
