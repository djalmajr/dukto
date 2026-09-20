import { expect, mock, test } from "bun:test";

test("platform pickers preserve mobile files and expose native media libraries", async () => {
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
		open: async (options: { pickerMode?: string }) => {
			dialogOptions.push(options);
			return options.pickerMode ? "file:///tmp/ios-photo.jpg" : "/tmp/desktop.txt";
		},
	}));

	try {
		const { pickSendItems, supportsFolderSelection, supportsMediaSelection } = await import(
			"./send-item-picker"
		);

		// Mutation captured: allowing iOS folder selection opens the unstable native directory picker.
		expect(supportsFolderSelection("ios")).toBe(false);
		expect(supportsFolderSelection("macos")).toBe(true);
		// Mutation captured: limiting media selection to iOS hides Android's native gallery flow.
		expect(supportsMediaSelection("android")).toBe(true);
		expect(supportsMediaSelection("ios")).toBe(true);
		expect(supportsMediaSelection("macos")).toBe(false);

		const androidFiles = await pickSendItems("android", false);
		expect(androidFiles).toEqual([
			{
				is_dir: false,
				name: "report.pdf",
				path: "/data/user/0/app.dukto/cache/dukto-selected/report.pdf",
				size: 42,
			},
		]);
		expect(invokes).toEqual([
			{ command: "open_send_file_picker", args: { media: false, multiple: true } },
		]);
		expect(dialogOptions).toEqual([]);

		await pickSendItems("android", false, true);
		expect(invokes.at(-1)).toEqual({
			command: "open_send_file_picker",
			args: { media: true, multiple: true },
		});

		const desktopFiles = await pickSendItems("macos", false);
		expect(desktopFiles).toEqual([
			{ is_dir: false, name: "desktop.txt", path: "/tmp/desktop.txt", size: 7 },
		]);
		expect(dialogOptions).toEqual([{ directory: false, multiple: true }]);
		expect(invokes.at(-1)).toEqual({
			command: "resolve_file_metadata",
			args: { paths: ["/tmp/desktop.txt"] },
		});

		await pickSendItems("ios", false);
		expect(dialogOptions.at(-1)).toEqual({
			directory: false,
			fileAccessMode: "copy",
			multiple: true,
			pickerMode: "document",
		});
		expect(invokes.at(-1)).toEqual({
			command: "resolve_file_metadata",
			args: { paths: ["file:///tmp/ios-photo.jpg"] },
		});

		const dialogCountBeforeFolderSelection = dialogOptions.length;
		expect(await pickSendItems("ios", true)).toBeNull();
		expect(dialogOptions).toHaveLength(dialogCountBeforeFolderSelection);

		await pickSendItems("ios", false, true);
		expect(dialogOptions.at(-1)).toEqual({
			directory: false,
			fileAccessMode: "copy",
			multiple: true,
			pickerMode: "media",
		});
	} finally {
		mock.restore();
	}
});
