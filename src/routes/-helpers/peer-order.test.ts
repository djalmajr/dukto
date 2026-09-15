import { expect, test } from "bun:test";
import { movePeer, orderedPeerIds } from "./peer-order";

test("discovery order does not replace the saved order; new hosts append", () => {
	expect(orderedPeerIds(["c", "b", "a", "new"], ["a", "offline", "b", "c"])).toEqual([
		"a",
		"b",
		"c",
		"new",
	]);
});

test("move preserves absent hosts and restores their position when they return", () => {
	const order = movePeer(["a", "offline", "b", "c"], ["a", "b", "c"], "c", "a", false);
	expect(order).toEqual(["c", "a", "offline", "b"]);
	expect(orderedPeerIds(["b", "offline", "a", "c"], order)).toEqual(order);
});

test("moves up and down, without duplicating IDs", () => {
	expect(movePeer(["a", "a", "b"], ["a", "b", "c"], "a", "c", true)).toEqual(["b", "c", "a"]);
	expect(movePeer([], ["a", "b", "c"], "c", "a", false)).toEqual(["c", "a", "b"]);
});

test("a host disappearing during drag cannot insert or remove a different host", () => {
	expect(movePeer(["a", "b", "c"], ["a", "c"], "b", "a", false)).toEqual(["a", "b", "c"]);
	expect(movePeer(["a", "b", "c"], ["a", "c"], "a", "b", true)).toEqual(["a", "b", "c"]);
});
