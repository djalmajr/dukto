import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export const releaseTargets = {
	"x86_64-pc-windows-msvc": {
		assetGroup: "windows_x64",
		bundleKind: "nsis",
		cliArchiveExtension: "zip",
		installerPattern: /-setup\.exe$/i,
		platform: "windows-x86_64",
		updaterAsset: (version) => `dukto_${version}_windows_x64-setup.exe`,
		updaterBundlePattern: /-setup\.exe$/i,
	},
	"aarch64-apple-darwin": {
		assetGroup: "macos_arm64",
		bundleKind: "macos",
		cliArchiveExtension: "tar.gz",
		installerPattern: /\.dmg$/i,
		platform: "darwin-aarch64",
		updaterAsset: (version) => `dukto_${version}_macos_arm64.app.tar.gz`,
		updaterBundlePattern: /\.app\.tar\.gz$/i,
	},
	"x86_64-apple-darwin": {
		assetGroup: "macos_x64",
		bundleKind: "macos",
		cliArchiveExtension: "tar.gz",
		installerPattern: /\.dmg$/i,
		platform: "darwin-x86_64",
		updaterAsset: (version) => `dukto_${version}_macos_x64.app.tar.gz`,
		updaterBundlePattern: /\.app\.tar\.gz$/i,
	},
	"x86_64-unknown-linux-gnu": {
		assetGroup: "linux_x64",
		bundleKind: "linux",
		cliArchiveExtension: "tar.gz",
		installerPattern: /\.deb$/i,
		platform: "linux-x86_64",
		updaterAsset: (version) => `dukto_${version}_linux_x64.AppImage`,
		updaterBundlePattern: /\.appimage$/i,
	},
	"aarch64-unknown-linux-gnu": {
		assetGroup: "linux_arm64",
		bundleKind: "linux",
		cliArchiveExtension: "tar.gz",
		installerPattern: /\.deb$/i,
		platform: "linux-aarch64",
		updaterAsset: (version) => `dukto_${version}_linux_arm64.AppImage`,
		updaterBundlePattern: /\.appimage$/i,
	},
};

async function listFiles(directory) {
	const entries = await readdir(directory, { withFileTypes: true }).catch((error) => {
		if (error.code === "ENOENT") return [];
		throw error;
	});
	const files = [];
	for (const entry of entries) {
		const path = join(directory, entry.name);
		if (entry.isDirectory()) files.push(...(await listFiles(path)));
		else if (entry.isFile()) files.push(path);
	}
	return files;
}

function requireOneFile(files, pattern, label) {
	const matches = files.filter((path) => pattern.test(basename(path)));
	if (matches.length !== 1) {
		throw new Error(`Expected exactly one ${label}, found ${matches.length}`);
	}
	return matches[0];
}

function requireVersionedAsset(path, version, label) {
	if (!basename(path).includes(`_${version}_`)) {
		throw new Error(
			`${label} filename does not match configured version ${version}: ${basename(path)}`,
		);
	}
}

async function copyAsset(source, outputDirectory, outputName) {
	const output = join(outputDirectory, outputName);
	await copyFile(source, output);
	return output;
}

export async function collectDesktopAssets({ target, base, out, version }) {
	const targetConfig = releaseTargets[target];
	if (!targetConfig) throw new Error("Pass a supported Rust target triple");
	const bundleFiles = await listFiles(join(base, "bundle"));
	const assets = [];
	const installer = requireOneFile(bundleFiles, targetConfig.installerPattern, "desktop installer");
	requireVersionedAsset(installer, version, "Desktop installer");

	if (targetConfig.bundleKind === "macos") {
		const updateBundle = requireOneFile(
			bundleFiles,
			targetConfig.updaterBundlePattern,
			"macOS updater archive",
		);
		const signature = `${updateBundle}.sig`;
		if (!(await stat(signature).catch(() => null))?.isFile()) {
			throw new Error(`Missing updater signature for ${basename(updateBundle)}`);
		}
		const arch = targetConfig.assetGroup === "macos_arm64" ? "arm64" : "x64";
		assets.push(await copyAsset(installer, out, `dukto_${version}_macos_${arch}.dmg`));
		assets.push(await copyAsset(updateBundle, out, targetConfig.updaterAsset(version)));
		assets.push(await copyAsset(signature, out, `${targetConfig.updaterAsset(version)}.sig`));
	} else if (target.startsWith("x86_64-pc-windows")) {
		const signature = `${installer}.sig`;
		if (!(await stat(signature).catch(() => null))?.isFile()) {
			throw new Error(`Missing updater signature for ${basename(installer)}`);
		}
		assets.push(await copyAsset(installer, out, targetConfig.updaterAsset(version)));
		assets.push(await copyAsset(signature, out, `${targetConfig.updaterAsset(version)}.sig`));
	} else {
		const updateBundle = requireOneFile(
			bundleFiles,
			targetConfig.updaterBundlePattern,
			"Linux AppImage updater asset",
		);
		const signature = `${updateBundle}.sig`;
		if (!(await stat(signature).catch(() => null))?.isFile()) {
			throw new Error(`Missing updater signature for ${basename(updateBundle)}`);
		}
		const arch = targetConfig.assetGroup === "linux_arm64" ? "arm64" : "x64";
		assets.push(await copyAsset(installer, out, `dukto_${version}_linux_${arch}.deb`));
		assets.push(await copyAsset(updateBundle, out, targetConfig.updaterAsset(version)));
		assets.push(await copyAsset(signature, out, `${targetConfig.updaterAsset(version)}.sig`));
	}

	for (const asset of assets) {
		if (asset.endsWith(".sig") && !(await readFile(asset, "utf8")).trim()) {
			throw new Error(`Updater signature is empty: ${basename(asset)}`);
		}
	}
	return assets;
}

async function createCliArchive({ base, out, target, version, targetConfig, readmePath }) {
	const windows = target.includes("windows");
	const sourceCli = windows ? "dukto-cli.exe" : "dukto-cli";
	const cli = windows ? "dukto.exe" : "dukto";
	const staging = join(out, "cli");
	await mkdir(staging, { recursive: true });
	await copyFile(join(base, sourceCli), join(staging, cli));
	await copyFile(readmePath, join(staging, "README.md"));
	const archive = join(
		out,
		`dukto-cli_${version}_${targetConfig.assetGroup}.${targetConfig.cliArchiveExtension}`,
	);
	if (windows) {
		execFileSync(
			"powershell.exe",
			[
				"-NoProfile",
				"-NonInteractive",
				"-Command",
				"Compress-Archive -LiteralPath $env:DUKTO_ARCHIVE_EXE,$env:DUKTO_ARCHIVE_README -DestinationPath $env:DUKTO_ARCHIVE_OUTPUT -Force",
			],
			{
				stdio: "inherit",
				windowsHide: true,
				env: {
					...process.env,
					DUKTO_ARCHIVE_EXE: join(staging, cli),
					DUKTO_ARCHIVE_README: join(staging, "README.md"),
					DUKTO_ARCHIVE_OUTPUT: archive,
				},
			},
		);
	} else {
		execFileSync("tar", ["-czf", archive, "-C", staging, cli, "README.md"], { stdio: "inherit" });
	}
	return archive;
}

export async function collectRelease({
	target,
	base,
	out,
	version,
	readmePath = join(ROOT, "docs/guides/cli.md"),
}) {
	const targetConfig = releaseTargets[target];
	if (!targetConfig) throw new Error("Pass a supported Rust target triple");
	await mkdir(out, { recursive: true });
	const cliArchive = await createCliArchive({
		base,
		out,
		target,
		version,
		targetConfig,
		readmePath,
	});
	const assets = [cliArchive, ...(await collectDesktopAssets({ target, base, out, version }))];
	const checksums = [];
	for (const asset of assets) {
		const hash = createHash("sha256")
			.update(await readFile(asset))
			.digest("hex");
		checksums.push(`${hash}  ${basename(asset)}`);
	}
	await writeFile(
		join(out, `SHA256SUMS-${targetConfig.assetGroup}.txt`),
		`${checksums.join("\n")}\n`,
	);
	console.log(`Collected ${assets.length} release assets and checksums in ${out}`);
}

async function main() {
	const target = process.argv[2];
	const targetConfig = releaseTargets[target];
	if (!targetConfig) throw new Error("Pass a supported Rust target triple");
	const pkg = JSON.parse(await readFile(join(ROOT, "package.json"), "utf8"));
	const config = JSON.parse(await readFile(join(ROOT, "src-tauri/tauri.conf.json"), "utf8"));
	if (pkg.version !== config.version) throw new Error("Package/Tauri version mismatch");
	const base = resolve(
		process.env.DUKTO_RELEASE_DIR || join(ROOT, `src-tauri/target/${target}/release`),
	);
	const out = resolve(join(ROOT, ".cache/release", target));
	await collectRelease({ target, base, out, version: pkg.version });
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	main().catch((error) => {
		console.error(error.message);
		process.exitCode = 1;
	});
}
