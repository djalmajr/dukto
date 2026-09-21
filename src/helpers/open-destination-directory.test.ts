import { expect, mock, test } from "bun:test";

test("opens the configured destination with the desktop shell", async () => {
	const opened: string[] = [];
	mock.module("@tauri-apps/plugin-shell", () => ({
		open: async (path: string) => {
			opened.push(path);
		},
	}));

	try {
		const { openDestinationDirectory } = await import("./open-destination-directory");
		await openDestinationDirectory("/Users/example/Shared");
		expect(opened).toEqual(["/Users/example/Shared"]);
	} finally {
		mock.restore();
	}
});
