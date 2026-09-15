import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { basename, join, resolve } from "node:path";

const target = process.argv[2];
const platforms = {
	"x86_64-pc-windows-msvc": "windows_x64",
	"aarch64-apple-darwin": "macos_arm64",
	"x86_64-apple-darwin": "macos_x64",
	"x86_64-unknown-linux-gnu": "linux_x64",
	"aarch64-unknown-linux-gnu": "linux_arm64",
};
if (!platforms[target]) throw new Error("Pass a supported Rust target triple");
const pkg = JSON.parse(await readFile("package.json", "utf8"));
const config = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
if (pkg.version !== config.version) throw new Error("Package/Tauri version mismatch");
const base = resolve(process.env.DUKTO_RELEASE_DIR || `src-tauri/target/${target}/release`);
const out = resolve(".cache/release", target);
const staging = join(out, "cli");
await mkdir(staging, { recursive: true });
const windows = target.includes("windows");
const sourceCli = windows ? "dukto-cli.exe" : "dukto-cli";
const cli = windows ? "dukto.exe" : "dukto";
await copyFile(join(base, sourceCli), join(staging, cli));
await copyFile("docs/guides/cli.md", join(staging, "README.md"));
const archive = join(
	out,
	`dukto-cli_${pkg.version}_${platforms[target]}.${windows ? "zip" : "tar.gz"}`,
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
} else
	execFileSync("tar", ["-czf", archive, "-C", staging, cli, "README.md"], { stdio: "inherit" });
const assets = [archive];
async function collect(dir) {
	for (const e of await readdir(dir, { withFileTypes: true })) {
		const p = join(dir, e.name);
		if (e.isDirectory()) await collect(p);
		else if (/\.(dmg|deb|AppImage)$|setup\.exe$/i.test(e.name)) {
			const dest = join(out, basename(p));
			await copyFile(p, dest);
			assets.push(dest);
		}
	}
}
await collect(join(base, "bundle"));
if (assets.length < 2) throw new Error("No desktop installer found");
const hashes = [];
for (const p of assets)
	hashes.push(
		`${createHash("sha256")
			.update(await readFile(p))
			.digest("hex")}  ${basename(p)}`,
	);
await writeFile(join(out, `SHA256SUMS-${platforms[target]}.txt`), `${hashes.join("\n")}\n`);
console.log(`Collected ${assets.length} artifacts in ${out}`);
