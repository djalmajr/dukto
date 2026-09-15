import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdir, mkdtemp, open, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { performance } from "node:perf_hooks";

// Real processes, real encrypted streams. A second request must progress before the first completes.
const binary = resolve(
	process.env.DUKTO_CLI ||
		`src-tauri/target/debug/dukto-cli${process.platform === "win32" ? ".exe" : ""}`,
);
const sizeMiB = Number(process.env.DUKTO_TEST_MIB || 512);
assert.ok(Number.isSafeInteger(sizeMiB) && sizeMiB >= 64 && sizeMiB <= 4096);
const root = await mkdtemp(join(tmpdir(), "dukto-concurrency-"));
const events = [];
const children = new Set();
const completions = [];
const started = performance.now();
let outcome;

function run(label, host, args) {
	const child = spawn(binary, ["--json", "--data-dir", join(root, host), ...args], {
		stdio: ["ignore", "pipe", "pipe"],
		windowsHide: true,
	});
	children.add(child);
	let buffer = "";
	let stderr = "";
	const records = [];
	child.stderr.on("data", (data) => {
		stderr += data;
	});
	child.stdout.on("data", (data) => {
		buffer += data;
		let newline = buffer.indexOf("\n");
		while (newline >= 0) {
			const line = buffer.slice(0, newline);
			buffer = buffer.slice(newline + 1);
			if (!line.trim()) continue;
			const record = {
				...JSON.parse(line),
				process: label,
				ms: Math.round(performance.now() - started),
			};
			records.push(record);
			events.push(record);
			newline = buffer.indexOf("\n");
		}
	});
	const done = new Promise((resolveDone, reject) => {
		child.on("error", reject);
		child.on("close", (code) => {
			children.delete(child);
			resolveDone({ code, stderr });
		});
	});
	completions.push(done);
	return { child, records, done };
}
async function waitFor(proc, predicate, timeout = 120000) {
	const deadline = performance.now() + timeout;
	while (performance.now() < deadline) {
		const found = proc.records.find(predicate);
		if (found) return found;
		if (proc.child.exitCode !== null)
			throw new Error(JSON.stringify({ records: proc.records, result: await proc.done }));
		await new Promise((resolveWait) => setTimeout(resolveWait, 20));
	}
	throw new Error(`Timed out waiting for event: ${JSON.stringify(proc.records)}`);
}
async function fixture(name, size) {
	const path = join(root, name);
	const block = randomBytes(1024 * 1024);
	const handle = await open(path, "wx");
	const hash = createHash("sha256");
	try {
		for (let remaining = size; remaining > 0; ) {
			const chunk = block.subarray(0, Math.min(remaining, block.length));
			await handle.writeFile(chunk);
			hash.update(chunk);
			remaining -= chunk.length;
		}
	} finally {
		await handle.close();
	}
	return { path, name, size, sha256: hash.digest("hex") };
}
async function hashFile(path) {
	const hash = createHash("sha256");
	for await (const chunk of createReadStream(path)) hash.update(chunk);
	return hash.digest("hex");
}
try {
	const first = await fixture("first-large.bin", sizeMiB * 1024 * 1024);
	const second = await fixture("second-large.bin", sizeMiB * 1024 * 1024);
	const reverse = await fixture("reverse-large.bin", sizeMiB * 1024 * 1024);
	const small = await fixture("later-small.bin", 65537);
	const receiverA = run("receiver-a", "host-a", [
		"receive",
		"--destination",
		join(root, "received-a"),
		"--port",
		"0",
		"--accept",
	]);
	const receiverB = run("receiver-b", "host-b", [
		"receive",
		"--destination",
		join(root, "received-b"),
		"--port",
		"0",
		"--accept",
	]);
	const readyA = await waitFor(receiverA, (e) => e.event === "listening");
	const readyB = await waitFor(receiverB, (e) => e.event === "listening");
	const send = (label, host, port, file) =>
		run(label, host, [
			"send",
			"--address",
			`127.0.0.1:${port}`,
			"--timeout",
			"180",
			"--",
			file.path,
		]);
	const sender1 = send("send-first", "host-a", readyB.data.port, first);
	const active = await waitFor(
		receiverB,
		(e) => e.event === "progress" && e.data.bytes_sent > 0 && e.data.percent < 100,
	);
	console.log(
		`First receive active at ${active.ms} ms (${active.data.percent.toFixed(1)}%). Starting two more sends and reverse traffic.`,
	);
	const sender2 = send("send-second", "host-a", readyB.data.port, second);
	const senderSmall = send("send-small", "host-a", readyB.data.port, small);
	const senderReverse = send("send-reverse", "host-b", readyA.data.port, reverse);
	const senders = [sender1, sender2, senderSmall, senderReverse];
	for (const sender of senders) {
		const result = await sender.done;
		assert.equal(result.code, 0, JSON.stringify(result));
		assert.ok(sender.records.some((e) => e.event === "sent" && e.data.acknowledged));
	}
	await waitFor(
		receiverB,
		() => receiverB.records.filter((e) => e.event === "received").length === 3,
	);
	await waitFor(receiverA, (e) => e.event === "received");
	for (const file of [first, second, small])
		assert.equal(await hashFile(join(root, "received-b", file.name)), file.sha256);
	assert.equal(await hashFile(join(root, "received-a", reverse.name)), reverse.sha256);
	const firstId = sender1.records.find((e) => e.event === "connecting").data.transfer_id;
	const secondId = sender2.records.find((e) => e.event === "connecting").data.transfer_id;
	const smallId = senderSmall.records.find((e) => e.event === "connecting").data.transfer_id;
	const firstComplete = receiverB.records.find(
		(e) => e.event === "received" && e.data.transfer_id === firstId,
	);
	const secondProgress = receiverB.records.find(
		(e) => e.event === "progress" && e.data.transfer_id === secondId && e.data.bytes_sent > 0,
	);
	const smallComplete = receiverB.records.find(
		(e) => e.event === "received" && e.data.transfer_id === smallId,
	);
	const reverseProgress = receiverA.records.find(
		(e) => e.event === "progress" && e.data.bytes_sent > 0,
	);
	outcome = {
		sizeMiB,
		hashesMatch: true,
		firstCompleteMs: firstComplete.ms,
		secondProgressMs: secondProgress?.ms,
		smallCompleteMs: smallComplete.ms,
		reverseProgressMs: reverseProgress?.ms,
		concurrentReceive: !!secondProgress && secondProgress.ms < firstComplete.ms,
		laterSmallFinishesFirst: smallComplete.ms < firstComplete.ms,
		bidirectionalOverlap: !!reverseProgress && reverseProgress.ms < firstComplete.ms,
	};
	console.log(JSON.stringify(outcome, null, 2));
	assert.ok(
		outcome.concurrentReceive,
		"Second large file must progress while first receive is active",
	);
	assert.ok(
		outcome.laterSmallFinishesFirst,
		"Later small transfer must not wait for earlier large transfer",
	);
	assert.ok(outcome.bidirectionalOverlap, "Same host must receive while sending");
	console.log("PASS concurrent large sends, later small send, bidirectional progress, all SHA-256");
	const survivor = send("send-survivor", "host-a", readyB.data.port, first);
	const survivorStart = await waitFor(survivor, (e) => e.event === "connecting");
	const survivorId = survivorStart.data.transfer_id;
	await waitFor(
		receiverB,
		(e) => e.event === "progress" && e.data.transfer_id === survivorId && e.data.percent < 100,
	);
	const canceled = send("send-canceled", "host-a", readyB.data.port, second);
	const canceledStart = await waitFor(canceled, (e) => e.event === "connecting");
	const canceledId = canceledStart.data.transfer_id;
	await waitFor(
		receiverB,
		(e) => e.event === "progress" && e.data.transfer_id === canceledId && e.data.percent < 100,
	);
	assert.ok(
		!receiverB.records.some((e) => e.event === "received" && e.data.transfer_id === survivorId),
		"Survivor must still be active when canceling its neighbor",
	);
	canceled.child.kill("SIGINT");
	const interrupted = await canceled.done;
	assert.notEqual(interrupted.code, 0);
	assert.ok(!canceled.records.some((e) => e.event === "sent"));
	const survived = await survivor.done;
	assert.equal(survived.code, 0, JSON.stringify(survived));
	await waitFor(receiverB, (e) => e.event === "received" && e.data.transfer_id === survivorId);
	const canceledReceive = await waitFor(
		receiverB,
		(e) => e.event === "receive_error" && e.data.transfer_id === canceledId,
	);
	// Windows child.kill force-terminates; POSIX SIGINT exercises the CLI's graceful shutdown.
	if (process.platform !== "win32") {
		assert.equal(canceledReceive.data.message, "Transfer cancelled by the remote peer");
	}
	const remainingFiles = await readdir(join(root, "received-b"));
	assert.ok(!remainingFiles.some((name) => name.startsWith(".dukto-partial-")));
	assert.ok(!remainingFiles.includes("second-large (1).bin"));
	assert.equal(await hashFile(join(root, "received-b", "first-large (1).bin")), first.sha256);
	assert.equal(await hashFile(join(root, "received-b", first.name)), first.sha256);
	outcome.cancellationIsolated = true;
	console.log(
		"PASS cancel one active sender; other transfer finishes with matching SHA-256 and receiver stays alive",
	);
	if (process.platform !== "win32") {
		const interruptedReceive = send("send-receiver-canceled", "host-a", readyB.data.port, first);
		const receiveStart = await waitFor(interruptedReceive, (e) => e.event === "connecting");
		await waitFor(
			receiverB,
			(e) =>
				e.event === "progress" &&
				e.data.transfer_id === receiveStart.data.transfer_id &&
				e.data.percent < 100,
		);
		receiverB.child.kill("SIGINT");
		assert.notEqual((await receiverB.done).code, 0);
		assert.notEqual((await interruptedReceive.done).code, 0);
		assert.ok(
			interruptedReceive.records.some(
				(e) => e.event === "error" && e.data.message === "Transfer cancelled by the remote peer",
			),
		);
		assert.ok(
			!(await readdir(join(root, "received-b"))).some((name) => name.startsWith(".dukto-partial-")),
		);
		assert.equal(await hashFile(join(root, "received-b", first.name)), first.sha256);
		outcome.receiverCancellationCleanedUp = true;
		console.log("PASS receiver Ctrl+C notifies sender and removes its incomplete file");
	}
} finally {
	for (const child of children) child.kill("SIGTERM");
	await Promise.allSettled(completions);
	if (process.env.DUKTO_TEST_REPORT) {
		const report = resolve(process.env.DUKTO_TEST_REPORT);
		await mkdir(resolve(report, ".."), { recursive: true });
		await writeFile(report, JSON.stringify({ outcome, events }, null, 2));
	}
	await rm(root, { recursive: true, force: true });
}
