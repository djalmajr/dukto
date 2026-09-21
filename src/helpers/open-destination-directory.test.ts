import { expect, mock, test } from "bun:test";

test("asks the backend to open the configured destination", async () => {
	const invokes: string[] = [];
	mock.module("@tauri-apps/api/core", () => ({
		invoke: async (command: string) => {
			invokes.push(command);
		},
	}));

	try {
		const { openDestinationDirectory } = await import("./open-destination-directory");
		await openDestinationDirectory();
		expect(invokes).toEqual(["open_destination_directory"]);
	} finally {
		mock.restore();
	}
});
