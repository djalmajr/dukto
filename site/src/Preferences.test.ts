import { describe, expect, test } from "bun:test";
import { isOutsideLanguagePicker } from "./language-picker";

describe("language picker", () => {
	test("distinguishes outside interactions from picker interactions", () => {
		const inside = new EventTarget();
		const outside = new EventTarget();
		const picker = {
			contains: (target: Node) => target === inside,
		};

		expect(isOutsideLanguagePicker(picker, inside)).toBe(false);
		expect(isOutsideLanguagePicker(picker, outside)).toBe(true);
		expect(isOutsideLanguagePicker(undefined, outside)).toBe(false);
		expect(isOutsideLanguagePicker(picker, null)).toBe(false);
	});
});
