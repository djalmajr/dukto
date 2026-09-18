import { expect, mock, test } from "bun:test";

test("destination selection uses the native Android tree picker and keeps the desktop dialog", async () => {
	const invokes: string[] = [];
	const dialogOptions: unknown[] = [];
	let destinationListener:
		| ((event: { payload: { path: string | null; error: string | null } }) => void)
		| null = null;

	mock.module("@tauri-apps/api/core", () => ({
		invoke: async (command: string) => {
			invokes.push(command);
			destinationListener?.({
				payload: { path: "/storage/emulated/0/Documents/Dukto", error: null },
			});
		},
	}));
	mock.module("@tauri-apps/api/event", () => ({
		listen: async (
			_event: string,
			listener: (event: { payload: { path: string | null; error: string | null } }) => void,
		) => {
			destinationListener = listener;
			return () => {
				destinationListener = null;
			};
		},
	}));
	mock.module("@tauri-apps/plugin-dialog", () => ({
		open: async (options: unknown) => {
			dialogOptions.push(options);
			return "/Users/test/Downloads";
		},
	}));

	try {
		const { pickDestinationDirectory } = await import("./destination-directory");

		expect(await pickDestinationDirectory("android")).toBe("/storage/emulated/0/Documents/Dukto");
		expect(invokes).toEqual(["open_destination_picker"]);
		expect(dialogOptions).toEqual([]);

		expect(await pickDestinationDirectory("macos")).toBe("/Users/test/Downloads");
		expect(dialogOptions).toEqual([{ directory: true, multiple: false }]);
	} finally {
		mock.restore();
	}
});
