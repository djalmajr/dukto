import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { collectDesktopAssets, releaseTargets } from "./collect-release.mjs";
import { generateUpdaterManifest } from "./generate-updater-manifest.mjs";

const version = "0.1.0";
const minisignAvailable = spawnSync("minisign", ["-v"], { stdio: "ignore" }).status === 0;
const copiedSignatureFixture = "synthetic copied signature fixture\n";
let artifactsDir;
let publicKey;
let secretKeyPath;

const targetAssets = [
	{
		checksum: "macos_arm64",
		files: [
			`dukto-cli_${version}_macos_arm64.tar.gz`,
			`dukto_${version}_macos_arm64.dmg`,
			`dukto_${version}_macos_arm64.app.tar.gz`,
			`dukto_${version}_macos_arm64.app.tar.gz.sig`,
		],
	},
	{
		checksum: "macos_x64",
		files: [
			`dukto-cli_${version}_macos_x64.tar.gz`,
			`dukto_${version}_macos_x64.dmg`,
			`dukto_${version}_macos_x64.app.tar.gz`,
			`dukto_${version}_macos_x64.app.tar.gz.sig`,
		],
	},
	{
		checksum: "windows_x64",
		files: [
			`dukto-cli_${version}_windows_x64.zip`,
			`dukto_${version}_windows_x64-setup.exe`,
			`dukto_${version}_windows_x64-setup.exe.sig`,
		],
	},
	{
		checksum: "linux_x64",
		files: [
			`dukto-cli_${version}_linux_x64.tar.gz`,
			`dukto_${version}_linux_x64.deb`,
			`dukto_${version}_linux_x64.AppImage`,
			`dukto_${version}_linux_x64.AppImage.sig`,
		],
	},
	{
		checksum: "linux_arm64",
		files: [
			`dukto-cli_${version}_linux_arm64.tar.gz`,
			`dukto_${version}_linux_arm64.deb`,
			`dukto_${version}_linux_arm64.AppImage`,
			`dukto_${version}_linux_arm64.AppImage.sig`,
		],
	},
];

async function createKeyPair({ publicPath, secretPath }) {
	execFileSync("minisign", ["-G", "-p", publicPath, "-s", secretPath, "-W"], {
		stdio: "ignore",
	});
	return Buffer.from(await readFile(publicPath)).toString("base64");
}

async function createSignature(assetPath, signaturePath) {
	execFileSync(
		"minisign",
		["-S", "-s", secretKeyPath, "-m", assetPath, "-x", signaturePath, "-W"],
		{ stdio: "ignore" },
	);
	const minisignFile = await readFile(signaturePath);
	await rm(signaturePath);
	return Buffer.from(minisignFile).toString("base64");
}

async function writeFixture({ omitFile } = {}) {
	for (const target of targetAssets) {
		const signatureName = target.files.find((name) => name.endsWith(".sig"));
		const updaterName = signatureName.slice(0, -".sig".length);
		for (const name of target.files.filter((file) => !file.endsWith(".sig"))) {
			await writeFile(join(artifactsDir, name), `fixture asset: ${name}\n`);
		}
		const signaturePath = join(artifactsDir, `${target.checksum}.minisig`);
		const signature = await createSignature(join(artifactsDir, updaterName), signaturePath);
		if (signatureName !== omitFile) await writeFile(join(artifactsDir, signatureName), signature);
		const checksums = [];
		for (const name of target.files) {
			const content = await readFile(join(artifactsDir, name)).catch(() => Buffer.from("omitted"));
			const hash = createHash("sha256").update(content).digest("hex");
			checksums.push(`${hash}  ${name}`);
		}
		await writeFile(
			join(artifactsDir, `SHA256SUMS-${target.checksum}.txt`),
			`${checksums.join("\n")}\n`,
		);
	}
}

function manifestInput() {
	return {
		artifactsDir,
		notes: "Dukto 0.1.0 release notes",
		repo: "djalmajr/dukto",
		tag: "v0.1.0",
		version,
		pubDate: "2026-09-14T12:00:00.000Z",
		publicKey,
	};
}

async function refreshChecksum(targetGroup, name) {
	const manifestPath = join(artifactsDir, `SHA256SUMS-${targetGroup}.txt`);
	const content = await readFile(join(artifactsDir, name));
	const digest = createHash("sha256").update(content).digest("hex");
	const lines = (await readFile(manifestPath, "utf8")).split(/\r?\n/);
	const index = lines.findIndex((line) => line.endsWith(`  ${name}`));
	if (index < 0) throw new Error(`Fixture checksum missing: ${name}`);
	lines[index] = `${digest}  ${name}`;
	await writeFile(manifestPath, `${lines.filter(Boolean).join("\n")}\n`);
}

async function createSourceBundle({ bundlePath, names }) {
	for (const [name, content] of names) {
		const path = join(bundlePath, name);
		await mkdir(dirname(path), { recursive: true });
		await writeFile(path, content);
	}
}

beforeEach(async () => {
	artifactsDir = await mkdtemp(join(tmpdir(), "dukto-updater-manifest-"));
});

afterEach(async () => {
	await rm(artifactsDir, { recursive: true, force: true });
});

describe.skipIf(!minisignAvailable)("generateUpdaterManifest", () => {
	beforeEach(async () => {
		secretKeyPath = join(artifactsDir, "test-private.key");
		publicKey = await createKeyPair({
			publicPath: join(artifactsDir, "test-public.key"),
			secretPath: secretKeyPath,
		});
	});
	test("rejects a stale installer whose version only contains the expected version as a substring", async () => {
		const base = join(artifactsDir, "stale-source");
		const out = join(artifactsDir, "stale-output");
		await mkdir(out);
		await createSourceBundle({
			bundlePath: join(base, "bundle"),
			names: [
				["nsis/Dukto_10.1.0_x64-setup.exe", "stale installer"],
				["nsis/Dukto_10.1.0_x64-setup.exe.sig", copiedSignatureFixture],
			],
		});
		await expect(
			collectDesktopAssets({ target: "x86_64-pc-windows-msvc", base, out, version }),
		).rejects.toThrow("filename does not match configured version");
	});

	test("collects the five platform installers and their paired updater signatures", async () => {
		const source = join(artifactsDir, "source");
		const output = join(artifactsDir, "collected");
		await mkdir(output);
		const bundles = [
			{
				target: "aarch64-apple-darwin",
				names: [
					["macos/Dukto_0.1.0_arm64.dmg", "macOS installer"],
					["macos/Dukto.app.tar.gz", "macOS update bundle"],
					["macos/Dukto.app.tar.gz.sig", copiedSignatureFixture],
				],
			},
			{
				target: "x86_64-apple-darwin",
				names: [
					["macos/Dukto_0.1.0_x64.dmg", "macOS installer"],
					["macos/Dukto.app.tar.gz", "macOS update bundle"],
					["macos/Dukto.app.tar.gz.sig", copiedSignatureFixture],
				],
			},
			{
				target: "x86_64-pc-windows-msvc",
				names: [
					["nsis/Dukto_0.1.0_x64-setup.exe", "Windows installer"],
					["nsis/Dukto_0.1.0_x64-setup.exe.sig", copiedSignatureFixture],
				],
			},
			{
				target: "x86_64-unknown-linux-gnu",
				names: [
					["deb/Dukto_0.1.0_amd64.deb", "Linux installer"],
					["appimage/Dukto_0.1.0_amd64.AppImage", "Linux update bundle"],
					["appimage/Dukto_0.1.0_amd64.AppImage.sig", copiedSignatureFixture],
				],
			},
			{
				target: "aarch64-unknown-linux-gnu",
				names: [
					["deb/Dukto_0.1.0_arm64.deb", "Linux installer"],
					["appimage/Dukto_0.1.0_arm64.AppImage", "Linux update bundle"],
					["appimage/Dukto_0.1.0_arm64.AppImage.sig", copiedSignatureFixture],
				],
			},
		];

		for (const bundle of bundles) {
			const base = join(source, bundle.target);
			await createSourceBundle({ bundlePath: join(base, "bundle"), names: bundle.names });
			const collected = await collectDesktopAssets({
				base,
				out: output,
				target: bundle.target,
				version,
			});
			expect(collected.length).toBe(bundle.target.startsWith("x86_64-pc-windows") ? 2 : 3);
		}

		expect(await Bun.file(join(output, `dukto_${version}_macos_arm64.app.tar.gz.sig`)).text()).toBe(
			copiedSignatureFixture,
		);
		expect(await Bun.file(join(output, `dukto_${version}_windows_x64-setup.exe.sig`)).text()).toBe(
			copiedSignatureFixture,
		);
		expect(await Bun.file(join(output, `dukto_${version}_linux_arm64.AppImage.sig`)).text()).toBe(
			copiedSignatureFixture,
		);

		const windowsBundle = bundles.find((bundle) => bundle.target.startsWith("x86_64-pc-windows"));
		const windowsBase = join(source, windowsBundle.target);
		const signaturePath = join(windowsBase, "bundle/nsis/Dukto_0.1.0_x64-setup.exe.sig");
		await rm(signaturePath);
		await expect(
			collectDesktopAssets({
				base: windowsBase,
				out: output,
				target: windowsBundle.target,
				version,
			}),
		).rejects.toThrow("Missing updater signature for Dukto_0.1.0_x64-setup.exe");
	});

	test("creates a complete, checksummed feed for all five target triples", async () => {
		await writeFixture();
		const manifest = await generateUpdaterManifest(manifestInput());
		expect(manifest.version).toBe(version);
		expect(manifest.notes).toBe("Dukto 0.1.0 release notes");
		expect(manifest.pub_date).toBe("2026-09-14T12:00:00.000Z");
		expect(Object.keys(manifest.platforms).sort()).toEqual([
			"darwin-aarch64",
			"darwin-x86_64",
			"linux-aarch64",
			"linux-x86_64",
			"windows-x86_64",
		]);
		expect(manifest.platforms["darwin-aarch64"]).toEqual({
			signature: (
				await readFile(join(artifactsDir, `dukto_${version}_macos_arm64.app.tar.gz.sig`), "utf8")
			).trim(),
			url: "https://github.com/djalmajr/dukto/releases/download/v0.1.0/dukto_0.1.0_macos_arm64.app.tar.gz",
		});
		expect(manifest.platforms["windows-x86_64"].url).toBe(
			"https://github.com/djalmajr/dukto/releases/download/v0.1.0/dukto_0.1.0_windows_x64-setup.exe",
		);
		expect(manifest.platforms["linux-aarch64"].url).toBe(
			"https://github.com/djalmajr/dukto/releases/download/v0.1.0/dukto_0.1.0_linux_arm64.AppImage",
		);
	});

	test("rejects a feed when any updater asset and signature pair is incomplete", async () => {
		await writeFixture({ omitFile: `dukto_${version}_windows_x64-setup.exe.sig` });
		await expect(generateUpdaterManifest(manifestInput())).rejects.toThrow(
			"Missing release asset: dukto_0.1.0_windows_x64-setup.exe.sig",
		);
	});

	test("rejects release assets whose checksums no longer match", async () => {
		await writeFixture();
		await writeFile(join(artifactsDir, `dukto_${version}_macos_arm64.app.tar.gz`), "altered");
		await expect(generateUpdaterManifest(manifestInput())).rejects.toThrow(
			"Checksum mismatch for dukto_0.1.0_macos_arm64.app.tar.gz",
		);
	});

	test("rejects a tampered updater asset even when its checksum file is updated", async () => {
		await writeFixture();
		const assetName = `dukto_${version}_macos_arm64.app.tar.gz`;
		await writeFile(join(artifactsDir, assetName), "tampered after signing");
		await refreshChecksum("macos_arm64", assetName);
		await expect(generateUpdaterManifest(manifestInput())).rejects.toThrow(
			`Minisign signature verification failed for ${assetName} (aarch64-apple-darwin)`,
		);
	});

	test("rejects valid signatures made by a key other than the configured updater public key", async () => {
		await writeFixture();
		const wrongPublicKey = await createKeyPair({
			publicPath: join(artifactsDir, "wrong-public.key"),
			secretPath: join(artifactsDir, "wrong-private.key"),
		});
		await expect(
			generateUpdaterManifest({ ...manifestInput(), publicKey: wrongPublicKey }),
		).rejects.toThrow(
			`Minisign signature verification failed for dukto_${version}_windows_x64-setup.exe (x86_64-pc-windows-msvc)`,
		);
	});

	test("rejects a tag whose version differs from the configured app version", async () => {
		await expect(generateUpdaterManifest({ ...manifestInput(), tag: "v0.1.1" })).rejects.toThrow(
			"Release tag v0.1.1 must match configured version v0.1.0",
		);
	});

	test("rejects prerelease versions from the stable updater feed", async () => {
		await expect(
			generateUpdaterManifest({
				...manifestInput(),
				version: "0.1.0-rc.1",
				tag: "v0.1.0-rc.1",
			}),
		).rejects.toThrow("Only stable semantic versions can generate latest.json: 0.1.0-rc.1");
	});
});
