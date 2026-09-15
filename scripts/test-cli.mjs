import assert from "node:assert/strict";
// Real CLI processes over QUIC + Noise; hashes and directory trees are the oracle.
// Run after: cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --bin dukto-cli
import { spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { once } from "node:events";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

const binary = resolve(
	process.env.DUKTO_CLI ||
		`src-tauri/target/debug/dukto-cli${process.platform === "win32" ? ".exe" : ""}`,
);
const root = await mkdtemp(join(tmpdir(), "dukto-cli-test-"));
const children = new Set();
function run(args, identity = "shared") {
	const child = spawn(binary, ["--json", "--data-dir", join(root, identity), ...args], {
		stdio: ["ignore", "pipe", "pipe"],
		windowsHide: true,
	});
	children.add(child);
	let stdout = "";
	let stderr = "";
	child.stdout.on("data", (b) => {
		stdout += b;
	});
	child.stderr.on("data", (b) => {
		stderr += b;
	});
	const done = once(child, "close").then(([code]) => {
		children.delete(child);
		return { code, stdout, stderr };
	});
	const timer = setTimeout(() => child.kill(), 45000);
	done.finally(() => clearTimeout(timer));
	return { child, done, output: () => stdout };
}
function records(stdout) {
	return stdout
		.split("\n")
		.filter((line) => line.trim())
		.map((line) => JSON.parse(line));
}
async function waitReady(proc) {
	const deadline = Date.now() + 10000;
	while (Date.now() < deadline) {
		const line = proc
			.output()
			.split("\n")
			.find((s) => s.includes('"event":"listening"'));
		if (line) {
			try {
				return JSON.parse(line).data;
			} catch {}
		}
		if (proc.child.exitCode !== null) throw new Error(JSON.stringify(await proc.done));
		await new Promise((r) => setTimeout(r, 30));
	}
	throw new Error(`Receiver not ready: ${proc.output()}`);
}
async function tree(path, prefix = "") {
	const entries = [];
	for (const entry of await readdir(path, { withFileTypes: true })) {
		const rel = prefix + entry.name;
		if (entry.isDirectory()) {
			entries.push({ path: `${rel}/`, directory: true });
			entries.push(...(await tree(join(path, entry.name), `${rel}/`)));
		} else {
			const data = await readFile(join(path, entry.name));
			entries.push({
				path: rel,
				size: data.length,
				sha256: createHash("sha256").update(data).digest("hex"),
			});
		}
	}
	return entries.sort((a, b) => a.path.localeCompare(b.path));
}
async function file(base, name, bytes) {
	const p = join(base, name);
	await mkdir(dirname(p), { recursive: true });
	await writeFile(p, bytes);
}

try {
	const cases = [
		{
			name: "single",
			roots: ["large.bin"],
			files: { "large.bin": randomBytes(2 * 1024 * 1024 + 17) },
			dirs: [],
		},
		{
			name: "multiple",
			roots: ["a.txt", "ação com espaços.txt", "empty.txt", "binary.bin"],
			files: {
				"a.txt": Buffer.from("Dukto\n"),
				"ação com espaços.txt": Buffer.from("Olá, macOS e Windows!"),
				"empty.txt": Buffer.alloc(0),
				"binary.bin": randomBytes(65537),
			},
			dirs: [],
		},
		{
			name: "folder",
			roots: ["project"],
			files: {
				"project/root.txt": Buffer.from("root"),
				"project/sub/deep/file.bin": randomBytes(131073),
			},
			dirs: ["project/empty"],
		},
		{
			name: "mixed",
			roots: ["loose.txt", "empty.txt", "alpha", "beta", "empty-root"],
			files: {
				"loose.txt": Buffer.from("Mixed files and folders"),
				"empty.txt": Buffer.alloc(0),
				"alpha/ação com espaços.txt": Buffer.from("Olá!"),
				"beta/sub/nested.bin": randomBytes(131079),
			},
			dirs: ["alpha/empty", "empty-root"],
		},
		{
			name: "folders",
			roots: ["alpha", "beta", "empty-root"],
			files: {
				"alpha/same.txt": Buffer.from("alpha"),
				"beta/same.txt": Buffer.from("beta"),
				"beta/sub/nested.bin": randomBytes(32769),
			},
			dirs: ["alpha/empty", "empty-root"],
		},
	];
	for (const c of cases) {
		const senderIdentityDir = `${c.name}-sender`;
		const receiverIdentityDir = `${c.name}-receiver`;
		const source = join(root, c.name, "source");
		const destination = join(root, c.name, "received");
		await mkdir(source, { recursive: true });
		for (const d of c.dirs) await mkdir(join(source, d), { recursive: true });
		for (const [name, data] of Object.entries(c.files)) await file(source, name, data);
		const expected = await tree(source);
		const receiver = run(
			["receive", "--destination", destination, "--port", "0", "--accept", "--once"],
			receiverIdentityDir,
		);
		const ready = await waitReady(receiver);
		const sender = run(
			[
				"send",
				"--address",
				`127.0.0.1:${ready.port}`,
				"--",
				...c.roots.map((p) => join(source, p)),
			],
			senderIdentityDir,
		);
		const [sent, received] = await Promise.all([sender.done, receiver.done]);
		assert.equal(sent.code, 0, JSON.stringify(sent));
		assert.equal(received.code, 0, JSON.stringify(received));
		assert.match(sent.stdout, /"acknowledged":true/);
		assert.match(received.stdout, /"event":"received"/);
		const incoming = records(received.stdout).find((record) => record.event === "incoming");
		assert.ok(incoming, "receiver reports the transfer header before acceptance");
		assert.equal(
			incoming.data.sender.device_id,
			(await readFile(join(root, senderIdentityDir, "device_id.txt"), "utf8")).trim(),
		);
		assert.match(incoming.data.sender.display_name, /\(CLI\)$/);
		assert.ok(incoming.data.sender.hostname);
		assert.equal(
			incoming.data.sender.platform,
			process.platform === "darwin" ? "macos" : process.platform === "win32" ? "windows" : "linux",
		);
		assert.deepEqual(await tree(destination), expected);
		console.log(`PASS ${c.name}: tree, sizes and SHA-256 match`);
	}
	const denied = await run(["receive", "--destination", join(root, "denied"), "--once"]).done;
	assert.notEqual(denied.code, 0);
	assert.match(denied.stdout, /--accept/);
	const missing = await run(["send", "--address", "127.0.0.1:1", join(root, "does-not-exist")])
		.done;
	assert.notEqual(missing.code, 0);
	console.log("PASS non-interactive approval and missing input return nonzero");
} finally {
	for (const child of children) child.kill();
	await rm(root, { recursive: true, force: true });
}
