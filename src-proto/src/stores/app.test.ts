import { beforeEach, describe, expect, test } from "bun:test";
import {
	abortTransfer,
	clearAllTransfers,
	dismissTransfer,
	failTransfer,
	getPeerTransfers,
	startTransfer,
	tickAllTransfers,
	transfers,
} from "./app";

beforeEach(() => {
	clearAllTransfers();
});

describe("proto transfer store", () => {
	test("startTransfer creates an active transfer", () => {
		const id = startTransfer("p1", "send", 1000, "10 MB/s");
		expect(id).toContain("p1-send-");
		const slot = transfers[id];
		expect(slot.status).toBe("active");
		expect(slot.direction).toBe("send");
		expect(slot.percent).toBe(0);
		expect(slot.bytesSent).toBe(0);
		expect(slot.bytesTotal).toBe(1000);
		expect(slot.speed).toBe("10 MB/s");
	});

	test("getPeerTransfers returns slots for a peer", () => {
		startTransfer("p1", "send", 1000);
		startTransfer("p1", "receive", 2000);
		startTransfer("p2", "send", 500);

		const p1 = getPeerTransfers("p1");
		expect(p1).toBeDefined();
		expect(p1?.length).toBe(2);

		const p2 = getPeerTransfers("p2");
		expect(p2?.length).toBe(1);

		expect(getPeerTransfers("p3")).toBeUndefined();
	});

	test("multiple transfers per direction are supported", () => {
		startTransfer("p1", "send", 1000);
		startTransfer("p1", "send", 2000);
		startTransfer("p1", "send", 3000);

		const slots = getPeerTransfers("p1");
		expect(slots?.length).toBe(3);
		expect(slots?.every((s) => s.direction === "send")).toBe(true);
	});

	test("dismissTransfer removes a specific transfer", () => {
		const id1 = startTransfer("p1", "send", 1000);
		const id2 = startTransfer("p1", "receive", 2000);

		dismissTransfer(id1);
		const slots = getPeerTransfers("p1");
		expect(slots?.length).toBe(1);
		expect(slots?.[0].id).toBe(id2);
	});

	test("abortTransfer removes an active transfer", () => {
		const id = startTransfer("p1", "send", 1000);
		expect(getPeerTransfers("p1")).toBeDefined();

		abortTransfer(id);
		expect(getPeerTransfers("p1")).toBeUndefined();
	});

	test("failTransfer creates an error transfer", () => {
		failTransfer("p1", "send", "Connection refused");

		const slots = getPeerTransfers("p1");
		expect(slots?.length).toBe(1);
		expect(slots?.[0].status).toBe("error");
		expect(slots?.[0].errorMsg).toBe("Connection refused");
	});

	test("tickAllTransfers advances active transfers", () => {
		const id = startTransfer("p1", "send", 1000);

		tickAllTransfers();
		const slot = transfers[id];
		expect(slot.percent).toBeGreaterThan(0);
		expect(slot.bytesSent).toBeGreaterThan(0);
	});

	test("tickAllTransfers completes at 100%", () => {
		const id = startTransfer("p1", "send", 1000);

		for (let i = 0; i < 20; i++) tickAllTransfers();

		const slot = transfers[id];
		expect(slot.percent).toBe(100);
		expect(slot.status).toBe("complete");
		expect(slot.completedAt).toBeDefined();
	});

	test("tickAllTransfers does not advance error transfers", () => {
		failTransfer("p1", "send", "Error");
		const slots = getPeerTransfers("p1");
		const errorId = slots?.[0].id;
		if (!errorId) throw new Error("Expected an error transfer");

		tickAllTransfers();
		const after = transfers[errorId];
		expect(after.status).toBe("error");
		expect(after.percent).toBe(0);
	});

	test("clearAllTransfers removes everything", () => {
		startTransfer("p1", "send", 1000);
		startTransfer("p2", "receive", 2000);

		clearAllTransfers();
		expect(Object.keys(transfers).length).toBe(0);
	});
});
