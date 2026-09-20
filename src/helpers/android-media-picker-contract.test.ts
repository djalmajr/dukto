import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";

const androidPickerSource = readFileSync(
	new URL("../../src-tauri/android/DestinationPlugin.kt", import.meta.url),
	"utf8",
);

test("Android media selection uses the native multi-select photo picker", () => {
	// Mutation captured: falling back to ACTION_OPEN_DOCUMENT loses the gallery-first media flow.
	expect(androidPickerSource).toContain("PickMultipleVisualMedia");
	expect(androidPickerSource).toContain("PickVisualMediaRequest");
	expect(androidPickerSource).toContain("ImageAndVideo");
	expect(androidPickerSource).toContain("if (args.media)");
});
