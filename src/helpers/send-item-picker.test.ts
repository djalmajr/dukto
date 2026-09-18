import { expect, mock, test } from "bun:test";

test("Android selection uses the native picker while desktop selection resolves dialog paths", async () => {
	const invokes: Array<{ command: string; args?: unknown }> = [];
	const dialogOptions: unknown[] = [];
	let filesListener:
		| ((event: {
				payload: {
					files: Array<{ is_dir: boolean; name: string; path: string; size: number }> | null;
					error: string | null;
				};
		  }) => void)
		| null = null;

	mock.module("@tauri-apps/api/core", () => ({
		invoke: async (command: string, args?: unknown) => {
			invokes.push({ command, args });
			if (command === "open_send_file_picker") {
				filesListener?.({
					payload: {
						files: [
							{
								is_dir: false,
								name: "report.pdf",
								path: "/data/user/0/app.dukto/cache/dukto-selected/report.pdf",
								size: 42,
							},
						],
						error: null,
					},
				});
			}
			if (command === "resolve_file_metadata") {
				return [{ is_dir: false, name: "desktop.txt", path: "/tmp/desktop.txt", size: 7 }];
			}
		},
	}));
	mock.module("@tauri-apps/api/event", () => ({
		listen: async (
			_event: string,
			listener: (event: {
				payload: {
					files: Array<{ is_dir: boolean; name: string; path: string; size: number }> | null;
					error: string | null;
				};
			}) => void,
		) => {
			filesListener = listener;
			return () => {
				filesListener = null;
			};
		},
	}));
	mock.module("@tauri-apps/plugin-dialog", () => ({
		open: async (options: unknown) => {
			dialogOptions.push(options);
			return "/tmp/desktop.txt";
		},
	}));

	try {
		const { pickSendItems } = await import("./send-item-picker");

		const androidFiles = await pickSendItems("android", false);
		expect(androidFiles).toEqual([
			{
				is_dir: false,
				name: "report.pdf",
				path: "/data/user/0/app.dukto/cache/dukto-selected/report.pdf",
				size: 42,
			},
		]);
		expect(invokes).toEqual([{ command: "open_send_file_picker", args: { multiple: true } }]);
		expect(dialogOptions).toEqual([]);

		const desktopFiles = await pickSendItems("macos", false);
		expect(desktopFiles).toEqual([
			{ is_dir: false, name: "desktop.txt", path: "/tmp/desktop.txt", size: 7 },
		]);
		expect(dialogOptions).toEqual([{ directory: false, multiple: true }]);
		expect(invokes.at(-1)).toEqual({
			command: "resolve_file_metadata",
			args: { paths: ["/tmp/desktop.txt"] },
		});
	} finally {
		mock.restore();
	}
});
