#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createReadStream, createWriteStream } from "node:fs";
import { chmod, mkdir, mkdtemp, readdir, rename, rm, stat } from "node:fs/promises";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

function usage() {
	console.error(
		"Usage: node scripts/patch-appimage.mjs <x86_64-unknown-linux-gnu|aarch64-unknown-linux-gnu> <AppImage> [Tauri cache directory]",
	);
	process.exitCode = 2;
}

export function appImageArchitecture(target) {
	if (target === "x86_64-unknown-linux-gnu") return "x86_64";
	if (target === "aarch64-unknown-linux-gnu") return "aarch64";
	throw new Error(`Unsupported Linux AppImage target: ${target}`);
}

async function findBundledWaylandClientLibraries(appDir) {
	const libraryRoot = join(appDir, "usr", "lib");
	const matches = [];
	async function visit(directory) {
		const entries = await readdir(directory, { withFileTypes: true }).catch((error) => {
			if (error.code === "ENOENT") return [];
			throw error;
		});
		for (const entry of entries) {
			const path = join(directory, entry.name);
			if (entry.isDirectory()) await visit(path);
			else if (
				(entry.isFile() || entry.isSymbolicLink()) &&
				entry.name.startsWith("libwayland-client.so.0")
			) {
				matches.push(path);
			}
		}
	}
	await visit(libraryRoot);
	return matches;
}

async function selectAppImageTool(cacheDir, architecture) {
	const files = await readdir(cacheDir).catch((error) => {
		if (error.code === "ENOENT") {
			throw new Error(`Tauri cache directory not found: ${cacheDir}`);
		}
		throw error;
	});
	const candidates = files
		.filter((name) => name.startsWith("linuxdeploy-plugin-appimage") && name.endsWith(".AppImage"))
		.filter(
			(name) => name === "linuxdeploy-plugin-appimage.AppImage" || name.includes(architecture),
		)
		.map((name) => join(cacheDir, name));
	if (candidates.length !== 1) {
		throw new Error(
			`Expected one linuxdeploy AppImage plugin for ${architecture} in ${cacheDir}, found ${candidates.length}`,
		);
	}
	return candidates[0];
}

async function extractAppImage(appImagePath, destination) {
	execFileSync(appImagePath, ["--appimage-extract"], {
		cwd: destination,
		env: { ...process.env, APPIMAGE_EXTRACT_AND_RUN: "1" },
		stdio: "ignore",
	});
	const appDir = join(destination, "squashfs-root");
	const appDirStat = await stat(appDir).catch(() => null);
	if (!appDirStat?.isDirectory()) {
		throw new Error(`AppImage did not extract to ${appDir}`);
	}
	return appDir;
}

async function extractRuntime(appImagePath, destination) {
	const offsetOutput = execFileSync(appImagePath, ["--appimage-offset"], {
		encoding: "utf8",
		stdio: ["ignore", "pipe", "inherit"],
	}).trim();
	if (!/^\d+$/.test(offsetOutput)) {
		throw new Error(`AppImage returned an invalid runtime offset: ${offsetOutput}`);
	}
	const offset = Number(offsetOutput);
	if (!Number.isSafeInteger(offset) || offset <= 0) {
		throw new Error(`AppImage returned an invalid runtime offset: ${offsetOutput}`);
	}
	const runtimePath = join(destination, "appimage-runtime");
	await pipeline(
		createReadStream(appImagePath, { start: 0, end: offset - 1 }),
		createWriteStream(runtimePath, { mode: 0o755 }),
	);
	return runtimePath;
}

async function replaceFileAtomically(source, destination, mode) {
	const staged = `${destination}.patched-${process.pid}`;
	try {
		await pipeline(
			createReadStream(source),
			createWriteStream(staged, { flags: "wx", mode: mode & 0o777 }),
		);
		await chmod(staged, mode);
		await rename(staged, destination);
	} finally {
		await rm(staged, { force: true });
	}
}

export async function patchAppImage({
	target,
	appImagePath,
	cacheDir = join(homedir(), ".cache", "tauri"),
}) {
	const architecture = appImageArchitecture(target);
	const imagePath = resolve(appImagePath);
	const imageStat = await stat(imagePath);
	if (!imageStat.isFile()) throw new Error(`AppImage is not a file: ${imagePath}`);
	const cachePath = resolve(cacheDir);
	const toolPath = await selectAppImageTool(cachePath, architecture);
	// Keep the large extraction and the staged replacement on the artifact's
	// filesystem; /tmp may be a small tmpfs or a separate mount on Linux.
	const tempDir = await mkdtemp(join(dirname(imagePath), ".dukto-appimage-patch-"));
	try {
		const sourceAppDir = await extractAppImage(imagePath, tempDir);
		const bundledLibraries = await findBundledWaylandClientLibraries(sourceAppDir);
		if (bundledLibraries.length === 0) {
			console.log(`${basename(imagePath)} already excludes bundled libwayland-client.so.0`);
			return false;
		}

		for (const library of bundledLibraries) await rm(library, { force: true });
		if ((await findBundledWaylandClientLibraries(sourceAppDir)).length > 0) {
			throw new Error("Failed to remove all bundled libwayland-client.so.0 files");
		}

		const runtimePath = await extractRuntime(imagePath, tempDir);
		const env = {
			...process.env,
			ARCH: architecture,
			APPIMAGE_EXTRACT_AND_RUN: "1",
			LDAI_RUNTIME_FILE: runtimePath,
		};
		execFileSync(toolPath, [`--appdir=${sourceAppDir}`], {
			cwd: tempDir,
			env,
			stdio: "inherit",
		});

		const repackedImages = (await readdir(tempDir))
			.filter((name) => name.endsWith(".AppImage"))
			.map((name) => join(tempDir, name));
		if (repackedImages.length !== 1) {
			throw new Error(`Expected one repacked AppImage, found ${repackedImages.length}`);
		}

		const verificationDir = join(tempDir, "verify");
		await mkdir(verificationDir);
		const finalAppDir = await extractAppImage(repackedImages[0], verificationDir);
		const remainingLibraries = await findBundledWaylandClientLibraries(finalAppDir);
		if (remainingLibraries.length > 0) {
			throw new Error(
				`Repacked AppImage still bundles libwayland-client: ${remainingLibraries.join(", ")}`,
			);
		}

		// A previous signature covers different bytes. Remove it before atomically
		// replacing the installer so it can never be mistaken for the final image.
		await rm(`${imagePath}.sig`, { force: true });
		await replaceFileAtomically(repackedImages[0], imagePath, imageStat.mode & 0o777);
		console.log(
			`Repacked ${basename(imagePath)} for host Mesa compatibility; removed ${bundledLibraries.length} bundled libwayland-client file(s) and invalidated its old updater signature`,
		);
		return true;
	} finally {
		await rm(tempDir, { recursive: true, force: true });
	}
}

async function main() {
	const [, , target, appImagePath, cacheDir] = process.argv;
	if (!target || !appImagePath || process.argv.length > 5) {
		usage();
		return;
	}
	await patchAppImage({ target, appImagePath, ...(cacheDir ? { cacheDir } : {}) });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
	main().catch((error) => {
		console.error(error instanceof Error ? error.message : error);
		process.exitCode = 1;
	});
}
