import { describe, expect, it } from "bun:test";
import {
	type AppUpdateDependencies,
	type AppUpdateHandle,
	type UpdateDownloadEvent,
	createAppUpdateController,
	getUpdateDownloadLabelKey,
} from "~/helpers/app-update-controller";

function deferred<T>() {
	let resolve: (value: T) => void = () => {};
	let reject: (reason: unknown) => void = () => {};
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, reject, resolve };
}

function makeUpdate(overrides: Partial<AppUpdateHandle> = {}) {
	const counts = { close: 0, download: 0, install: 0 };
	const update: AppUpdateHandle = {
		body: "Release notes for the candidate.",
		currentVersion: "0.1.0",
		version: "0.1.1",
		close: async () => {
			counts.close += 1;
		},
		download: async () => {
			counts.download += 1;
		},
		install: async () => {
			counts.install += 1;
		},
		...overrides,
	};
	return { counts, update };
}

function makeDependencies(overrides: Partial<AppUpdateDependencies> = {}) {
	const counts = { check: 0, getVersion: 0, relaunch: 0 };
	const dependencies: AppUpdateDependencies = {
		getVersion: async () => {
			counts.getVersion += 1;
			return "0.1.0";
		},
		isDevelopment: false,
		relaunch: async () => {
			counts.relaunch += 1;
		},
		...overrides,
		check: async () => {
			counts.check += 1;
			return overrides.check ? overrides.check() : null;
		},
	};
	return { counts, dependencies };
}

describe("app update controller", () => {
	it("deduplicates startup and manual checks while surfacing the result", async () => {
		// Mutation captured: allowing a second check while the first is pending creates duplicate update resources.
		const response = deferred<AppUpdateHandle | null>();
		const { counts, dependencies } = makeDependencies({ check: () => response.promise });
		const { update } = makeUpdate();
		const controller = createAppUpdateController(dependencies);
		const startup = controller.startupCheck();
		const manual = controller.checkForUpdates(false);

		response.resolve(update);
		await Promise.all([startup, manual]);
		expect(counts.check).toBe(1);
		expect(controller.state).toMatchObject({
			availableVersion: "0.1.1",
			currentVersion: "0.1.0",
			dialogOpen: true,
			releaseNotes: "Release notes for the candidate.",
			status: "available",
		});
		expect(counts.getVersion).toBe(1);
	});

	it("keeps startup failures silent and explains a missing release feed on manual check", async () => {
		// Regression: treating a missing release endpoint as up-to-date hides a broken release configuration.
		let checks = 0;
		const { dependencies } = makeDependencies({
			check: async () => {
				checks += 1;
				throw new Error("HTTP 404 Not Found");
			},
		});
		const controller = createAppUpdateController(dependencies);
		await controller.startupCheck();
		expect(controller.state).toMatchObject({ status: "idle", errorCode: null, errorMessage: null });
		await controller.checkForUpdates(false);
		expect(controller.state).toMatchObject({
			status: "checkError",
			errorCode: "feedUnavailable",
			errorMessage: "HTTP 404 Not Found",
		});
		expect(controller.state.status).not.toBe("upToDate");
		expect(checks).toBe(2);
	});

	it("defers an automatic update prompt while the incoming-transfer dialog owns focus", async () => {
		// Mutation captured: presenting the startup update immediately can stack two modal focus traps.
		const { update } = makeUpdate();
		const { dependencies } = makeDependencies({ check: async () => update });
		const controller = createAppUpdateController(dependencies);
		controller.setIncomingRequestActive(true);
		await controller.startupCheck();
		expect(controller.state).toMatchObject({ status: "available", dialogOpen: false });
		controller.setIncomingRequestActive(false);
		expect(controller.state.dialogOpen).toBe(true);
	});

	it("waits to present a manual check result until the incoming request is answered", async () => {
		// Mutation captured: a manual check resolving during a receive can open a later, higher modal over it.
		const started = deferred<void>();
		const response = deferred<AppUpdateHandle | null>();
		const { update } = makeUpdate();
		const { dependencies } = makeDependencies({
			check: () => {
				started.resolve();
				return response.promise;
			},
		});
		const controller = createAppUpdateController(dependencies);
		const request = controller.checkForUpdates(false);
		await started.promise;
		controller.setIncomingRequestActive(true);
		response.resolve(update);
		await request;
		expect(controller.state).toMatchObject({ status: "available", dialogOpen: false });
		controller.setIncomingRequestActive(false);
		expect(controller.state.dialogOpen).toBe(true);
	});

	it("keeps a manual download running behind incoming requests and resumes at complete progress", async () => {
		// Mutation captured: allowing update z-order to win blocks receive approval; partial progress on success is misleading.
		const downloadResponse = deferred<void>();
		let onDownloadEvent: ((event: UpdateDownloadEvent) => void) | undefined;
		const { counts: updateCounts, update } = makeUpdate({
			download: async (onEvent) => {
				updateCounts.download += 1;
				onDownloadEvent = onEvent;
				await downloadResponse.promise;
			},
		});
		const { dependencies } = makeDependencies({ check: async () => update });
		const controller = createAppUpdateController(dependencies);
		await controller.checkForUpdates(false);
		const firstDownload = controller.downloadUpdate();
		const secondDownload = controller.downloadUpdate();
		expect(firstDownload).toBe(secondDownload);
		expect(updateCounts.download).toBe(1);
		controller.setIncomingRequestActive(true);
		expect(controller.state).toMatchObject({ status: "downloading", dialogOpen: false });
		onDownloadEvent?.({ event: "Started", data: { contentLength: 100 } });
		onDownloadEvent?.({ event: "Progress", data: { chunkLength: 40 } });
		expect(controller.state.progress).toEqual({ downloaded: 40, percent: 40, total: 100 });
		downloadResponse.resolve();
		await firstDownload;
		expect(controller.state).toMatchObject({
			status: "downloaded",
			dialogOpen: false,
			progress: { downloaded: 100, percent: 100, total: 100 },
		});
		onDownloadEvent?.({ event: "Finished" });
		expect(controller.state.progress).toEqual({ downloaded: 100, percent: 100, total: 100 });
		controller.setIncomingRequestActive(false);
		expect(controller.state).toMatchObject({ status: "downloaded", dialogOpen: true });
		controller.setIncomingRequestActive(true);
		expect(controller.state.dialogOpen).toBe(false);
		controller.setIncomingRequestActive(false);
		expect(controller.state.dialogOpen).toBe(true);
	});

	it("keeps unknown download size indeterminate after successful completion", async () => {
		const { update } = makeUpdate({
			download: async (onEvent) => {
				onEvent({ event: "Progress", data: { chunkLength: 25 } });
			},
		});
		const { dependencies } = makeDependencies({ check: async () => update });
		const controller = createAppUpdateController(dependencies);
		await controller.checkForUpdates(false);
		await controller.downloadUpdate();
		expect(controller.state).toMatchObject({
			status: "downloaded",
			progress: { downloaded: 25, percent: null, total: null },
		});
	});

	it("does not reopen an update prompt dismissed with Later after an incoming request", async () => {
		// Mutation captured: a pending automatic presentation must be cleared when Later is chosen.
		const { update } = makeUpdate();
		const { dependencies } = makeDependencies({ check: async () => update });
		const controller = createAppUpdateController(dependencies);
		await controller.checkForUpdates(false);
		expect(controller.state.dialogOpen).toBe(true);
		controller.dismissUpdate();
		controller.setIncomingRequestActive(true);
		controller.setIncomingRequestActive(false);
		controller.presentDeferredUpdate();
		expect(controller.state.dialogOpen).toBe(false);
	});

	it("uses failure text instead of a completed label when download fails", () => {
		// Mutation captured: rendering a failed partial file as downloaded contradicts the error action.
		expect(getUpdateDownloadLabelKey("downloading")).toBe("updateDownloading");
		expect(getUpdateDownloadLabelKey("downloadError")).toBe("updateDownloadFailed");
		expect(getUpdateDownloadLabelKey("downloaded")).toBe("updateDownloaded");
	});

	it("retains the downloaded update when active transfers block installation and retries the same package", async () => {
		// Mutation captured: discarding the downloaded handle on a busy result forces a redundant download.
		const { counts: updateCounts, update } = makeUpdate({
			install: async () => {
				updateCounts.install += 1;
				if (updateCounts.install === 1) {
					throw { kind: "busy", message: "transfers and approvals are still active" };
				}
			},
		});
		const { counts, dependencies } = makeDependencies({ check: async () => update });
		const controller = createAppUpdateController(dependencies);
		await controller.checkForUpdates(false);
		await controller.downloadUpdate();
		await controller.installUpdate();
		expect(controller.state).toMatchObject({
			status: "waitingForTransfers",
			errorCode: "transferGate",
		});
		expect(updateCounts.install).toBe(1);
		expect(updateCounts.download).toBe(1);
		controller.dismissUpdate();
		expect(controller.state.dialogOpen).toBe(false);
		await controller.checkForUpdates(false);
		expect(controller.state.dialogOpen).toBe(true);
		expect(counts.check).toBe(1);
		await controller.installUpdate();
		expect(controller.state.status).toBe("restarting");
		expect(updateCounts.download).toBe(1);
		expect(updateCounts.install).toBe(2);
		expect(updateCounts.close).toBe(1);
		expect(counts.relaunch).toBe(1);
	});

	it("surfaces install errors and allows another explicit installation attempt", async () => {
		// Mutation captured: swallowing an install error leaves a busy-looking dialog with no recovery action.
		const { counts: updateCounts, update } = makeUpdate({
			install: async () => {
				updateCounts.install += 1;
				if (updateCounts.install === 1) {
					throw { kind: "install", message: "installer rejected package" };
				}
			},
		});
		const { counts, dependencies } = makeDependencies({ check: async () => update });
		const controller = createAppUpdateController(dependencies);
		await controller.checkForUpdates(false);
		await controller.downloadUpdate();
		await controller.installUpdate();
		expect(controller.state).toMatchObject({ status: "installError", errorCode: "install" });
		await controller.installUpdate();
		expect(controller.state.status).toBe("restarting");
		expect(updateCounts.download).toBe(1);
		expect(counts.relaunch).toBe(1);
	});

	it("keeps the transfer gate closed and blocks dismissal when relaunch fails after install", async () => {
		// Mutation captured: allowing another install or dismissing the failure hides the required manual restart.
		const { counts: updateCounts, update } = makeUpdate();
		const { counts, dependencies } = makeDependencies({
			check: async () => update,
			relaunch: async () => {
				counts.relaunch += 1;
				throw new Error("relaunch unavailable");
			},
		});
		const controller = createAppUpdateController(dependencies);
		await controller.checkForUpdates(false);
		await controller.downloadUpdate();
		await controller.installUpdate();
		controller.dismissUpdate();
		await controller.installUpdate();
		expect(controller.state).toMatchObject({
			status: "restartFailed",
			errorCode: "restart",
			dialogOpen: true,
		});
		expect(updateCounts.install).toBe(1);
		expect(counts.relaunch).toBe(1);
	});
});
