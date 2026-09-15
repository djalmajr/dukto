import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { releaseTargets } from "./collect-release.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const STABLE_SEMVER = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:\+[0-9A-Za-z.-]+)?$/;

function fail(message) {
	throw new Error(message);
}

function installerAsset({ target, targetConfig, version }) {
	if (target.startsWith("x86_64-pc-windows")) return targetConfig.updaterAsset(version);
	const arch = targetConfig.assetGroup.endsWith("arm64") ? "arm64" : "x64";
	const os = targetConfig.assetGroup.startsWith("macos") ? "macos" : "linux";
	return `dukto_${version}_${os}_${arch}.${os === "macos" ? "dmg" : "deb"}`;
}

function expectedTargetAssets({ target, targetConfig, version }) {
	const updater = targetConfig.updaterAsset(version);
	const installer = installerAsset({ target, targetConfig, version });
	return [
		`dukto-cli_${version}_${targetConfig.assetGroup}.${targetConfig.cliArchiveExtension}`,
		installer,
		...(installer === updater ? [] : [updater]),
		`${updater}.sig`,
	];
}

function parseChecksumManifest(contents, target) {
	const checksums = new Map();
	for (const line of contents.split(/\r?\n/).filter(Boolean)) {
		const match = /^([a-f\d]{64}) {2}([A-Za-z0-9._+-]+)$/i.exec(line);
		if (!match) fail(`Invalid checksum entry in ${target}: ${line}`);
		const [, digest, name] = match;
		if (checksums.has(name)) fail(`Duplicate checksum entry in ${target}: ${name}`);
		checksums.set(name, digest.toLowerCase());
	}
	return checksums;
}

async function readRegularAsset(artifactsDir, name) {
	const path = resolve(artifactsDir, name);
	if (dirname(path) !== resolve(artifactsDir)) fail(`Unsafe release asset name: ${name}`);
	const info = await lstat(path).catch(() => null);
	if (!info?.isFile()) fail(`Missing release asset: ${name}`);
	const contents = await readFile(path);
	if (contents.length === 0) fail(`Release asset is empty: ${name}`);
	return contents;
}

function decodeBase64Text(encoded, label) {
	const value = encoded.trim();
	if (
		!value ||
		!/^[A-Za-z0-9+/]+={0,2}$/.test(value) ||
		(value.includes("=") && value.length % 4 !== 0) ||
		(!value.includes("=") && value.length % 4 === 1)
	) {
		fail(`${label} is not valid base64`);
	}
	const bytes = Buffer.from(value, "base64");
	if (bytes.toString("base64").replace(/=+$/, "") !== value.replace(/=+$/, "")) {
		fail(`${label} is not valid base64`);
	}
	const decoded = bytes.toString("utf8");
	if (!Buffer.from(decoded, "utf8").equals(bytes)) fail(`${label} is not UTF-8 text`);
	return decoded;
}

async function verifyMinisign({
	artifactPath,
	updater,
	target,
	signature,
	publicKeyPath,
	verificationDir,
}) {
	const decodedSignature = decodeBase64Text(signature, `Updater signature for ${updater}`);
	const signaturePath = resolve(verificationDir, `${target}.minisig`);
	await writeFile(signaturePath, decodedSignature);
	try {
		execFileSync(
			"minisign",
			["-V", "-q", "-m", artifactPath, "-p", publicKeyPath, "-x", signaturePath],
			{ stdio: "ignore" },
		);
	} catch (error) {
		if (error?.code === "ENOENT") fail("minisign is required to verify updater signatures");
		fail(`Minisign signature verification failed for ${updater} (${target})`);
	}
}

async function validateTargetChecksums({
	artifactsDir,
	target,
	targetConfig,
	version,
	publicKeyPath,
	verificationDir,
}) {
	const expectedAssets = expectedTargetAssets({ target, targetConfig, version });
	const checksumName = `SHA256SUMS-${targetConfig.assetGroup}.txt`;
	const contents = await readFile(resolve(artifactsDir, checksumName), "utf8").catch(() => null);
	if (contents === null) fail(`Missing checksum manifest: ${checksumName}`);
	const checksums = parseChecksumManifest(contents, target);
	const expected = new Set(expectedAssets);
	if (checksums.size !== expected.size || [...expected].some((name) => !checksums.has(name))) {
		fail(`Checksum manifest ${checksumName} does not list the exact release assets for ${target}`);
	}
	for (const name of expectedAssets) {
		if (!name.includes(`_${version}_`)) fail(`Release asset version mismatch: ${name}`);
		const bytes = await readRegularAsset(artifactsDir, name);
		const actual = createHash("sha256").update(bytes).digest("hex");
		if (checksums.get(name) !== actual) fail(`Checksum mismatch for ${name}`);
	}
	const updater = targetConfig.updaterAsset(version);
	const signature = (await readRegularAsset(artifactsDir, `${updater}.sig`))
		.toString("utf8")
		.trim();
	if (!signature) fail(`Updater signature is empty: ${updater}.sig`);
	await verifyMinisign({
		artifactPath: resolve(artifactsDir, updater),
		updater,
		target,
		signature,
		publicKeyPath,
		verificationDir,
	});
	return { signature, updater };
}

export async function generateUpdaterManifest({
	artifactsDir,
	version,
	repo,
	tag,
	notes,
	pubDate,
	publicKey,
}) {
	if (!STABLE_SEMVER.test(version)) {
		fail(`Only stable semantic versions can generate latest.json: ${version}`);
	}
	if (tag !== `v${version}`) fail(`Release tag ${tag} must match configured version v${version}`);
	if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo)) fail(`Invalid GitHub repository: ${repo}`);
	if (typeof notes !== "string" || !notes.trim())
		fail("Release notes are required to generate latest.json");
	if (typeof publicKey !== "string" || !publicKey.trim())
		fail("Tauri updater public key is required to verify latest.json artifacts");
	const publishedAt = pubDate ?? new Date().toISOString();
	if (Number.isNaN(Date.parse(publishedAt)))
		fail(`Invalid RFC 3339 publication date: ${publishedAt}`);

	const targets = Object.entries(releaseTargets);
	if (targets.length !== 5) fail(`Expected five updater targets, configured ${targets.length}`);
	const baseUrl = `https://github.com/${repo}/releases/download/${tag}`;
	const platforms = {};
	const verificationDir = await mkdtemp(resolve(tmpdir(), "dukto-updater-signatures-"));
	try {
		const publicKeyText = decodeBase64Text(publicKey, "Tauri updater public key");
		if (!publicKeyText.includes("minisign public key")) {
			fail("Tauri updater public key is not a Minisign public key");
		}
		const publicKeyPath = resolve(verificationDir, "tauri-updater.pub");
		await writeFile(publicKeyPath, publicKeyText);
		for (const [target, targetConfig] of targets) {
			const { signature, updater } = await validateTargetChecksums({
				artifactsDir,
				target,
				targetConfig,
				version,
				publicKeyPath,
				verificationDir,
			});
			platforms[targetConfig.platform] = {
				signature,
				url: `${baseUrl}/${encodeURIComponent(updater)}`,
			};
		}
	} finally {
		await rm(verificationDir, { recursive: true, force: true });
	}
	return { version, notes: notes.trim(), pub_date: publishedAt, platforms };
}

function parseArguments(args) {
	const options = {};
	for (let index = 0; index < args.length; index += 2) {
		const key = args[index];
		const value = args[index + 1];
		if (!key?.startsWith("--") || !value || options[key])
			fail(`Invalid argument: ${key ?? "<missing>"}`);
		options[key] = value;
	}
	const required = ["--artifacts-dir", "--output", "--repo", "--tag", "--notes-file"];
	for (const key of required) if (!options[key]) fail(`Missing required argument: ${key}`);
	for (const key of Object.keys(options))
		if (!required.includes(key)) fail(`Unknown argument: ${key}`);
	return options;
}

async function main() {
	const options = parseArguments(process.argv.slice(2));
	const pkg = JSON.parse(await readFile(resolve(ROOT, "package.json"), "utf8"));
	const config = JSON.parse(await readFile(resolve(ROOT, "src-tauri/tauri.conf.json"), "utf8"));
	if (pkg.version !== config.version) fail("Package/Tauri version mismatch");
	const notes = await readFile(resolve(ROOT, options["--notes-file"]), "utf8");
	const manifest = await generateUpdaterManifest({
		artifactsDir: resolve(ROOT, options["--artifacts-dir"]),
		notes,
		repo: options["--repo"],
		tag: options["--tag"],
		version: pkg.version,
		publicKey: config.plugins?.updater?.pubkey,
	});
	const output = resolve(ROOT, options["--output"]);
	await mkdir(dirname(output), { recursive: true });
	await writeFile(output, `${JSON.stringify(manifest, null, 2)}\n`);
	console.log(
		`Generated a complete ${Object.keys(manifest.platforms).length}-platform feed at ${output}`,
	);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	main().catch((error) => {
		console.error(error.message);
		process.exitCode = 1;
	});
}
