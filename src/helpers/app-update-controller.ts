import { createStore } from "solid-js/store";

export type UpdateDownloadEvent =
	| { event: "Started"; data: { contentLength?: number } }
	| { event: "Progress"; data: { chunkLength: number } }
	| { event: "Finished"; data?: never };

export interface AppUpdateHandle {
	body?: string;
	currentVersion: string;
	version: string;
	download: (onEvent: (event: UpdateDownloadEvent) => void) => Promise<void>;
	install: () => Promise<void>;
	close: () => Promise<void>;
}

export type AppUpdateStatus =
	| "idle"
	| "checking"
	| "upToDate"
	| "checkError"
	| "available"
	| "downloading"
	| "downloaded"
	| "downloadError"
	| "preparingInstall"
	| "waitingForTransfers"
	| "installing"
	| "installError"
	| "restarting"
	| "restartFailed";

export type AppUpdateErrorCode =
	| "feedUnavailable"
	| "check"
	| "download"
	| "transferGate"
	| "install"
	| "restart"
	| null;

export interface AppUpdateProgress {
	downloaded: number;
	percent: number | null;
	total: number | null;
}

export interface AppUpdateState {
	availableVersion: string | null;
	currentVersion: string | null;
	dialogOpen: boolean;
	errorCode: AppUpdateErrorCode;
	errorMessage: string | null;
	progress: AppUpdateProgress | null;
	releaseNotes: string;
	status: AppUpdateStatus;
}

export interface AppUpdateDependencies {
	check: () => Promise<AppUpdateHandle | null>;
	getVersion: () => Promise<string>;
	isDevelopment: boolean;
	relaunch: () => Promise<void>;
}

export interface AppUpdateController {
	state: AppUpdateState;
	checkForUpdates: (silent?: boolean) => Promise<void>;
	dismissUpdate: () => void;
	downloadUpdate: () => Promise<void>;
	isBusy: () => boolean;
	installUpdate: () => Promise<void>;
	setIncomingRequestActive: (active: boolean) => void;
	presentDeferredUpdate: () => void;
	startupCheck: () => Promise<void>;
}

export type UpdateDownloadLabelKey =
	| "updateDownloading"
	| "updateDownloaded"
	| "updateDownloadFailed";

export function getUpdateDownloadLabelKey(status: AppUpdateStatus): UpdateDownloadLabelKey {
	if (status === "downloading") return "updateDownloading";
	if (status === "downloadError") return "updateDownloadFailed";
	return "updateDownloaded";
}

function errorText(error: unknown): string {
	if (error instanceof Error) return error.message;
	if (typeof error === "string") return error;
	try {
		return JSON.stringify(error) ?? String(error);
	} catch {
		return String(error);
	}
}

function isFeedUnavailable(error: unknown): boolean {
	return /(?:\b404\b|not found|no release found)/i.test(errorText(error));
}

function installFailure(error: unknown): { code: "install" | "transferGate"; message: string } {
	if (error && typeof error === "object" && "kind" in error && "message" in error) {
		const kind = error.kind;
		const message = typeof error.message === "string" ? error.message : errorText(error);
		if (kind === "busy") return { code: "transferGate", message };
		if (kind === "install") return { code: "install", message };
	}
	return { code: "install", message: errorText(error) };
}

function progressPercent(downloaded: number, total: number | null): number | null {
	if (!total || total <= 0) return null;
	return Math.min(100, Math.floor((downloaded / total) * 100));
}

function isBusyStatus(status: AppUpdateStatus): boolean {
	return (
		status === "checking" ||
		status === "downloading" ||
		status === "preparingInstall" ||
		status === "installing" ||
		status === "restarting"
	);
}

export function createAppUpdateController(
	dependencies: AppUpdateDependencies,
): AppUpdateController {
	const [state, setState] = createStore<AppUpdateState>({
		availableVersion: null,
		currentVersion: null,
		dialogOpen: false,
		errorCode: null,
		errorMessage: null,
		progress: null,
		releaseNotes: "",
		status: "idle",
	});
	let update: AppUpdateHandle | null = null;
	let versionRequest: Promise<void> | null = null;
	let checkRequest: Promise<void> | null = null;
	let downloadRequest: Promise<void> | null = null;
	let installRequest: Promise<void> | null = null;
	let manualCheckRequested = false;
	let startupCheckStarted = false;
	let presentationPending = false;
	let incomingRequestActive = false;

	function loadCurrentVersion(): Promise<void> {
		if (state.currentVersion) return Promise.resolve();
		if (versionRequest) return versionRequest;
		const request = dependencies
			.getVersion()
			.then((version) => setState("currentVersion", version))
			.catch(() => {});
		versionRequest = request;
		void request.finally(() => {
			if (versionRequest === request) versionRequest = null;
		});
		return request;
	}

	function isBusy(): boolean {
		return isBusyStatus(state.status);
	}

	function checkForUpdates(silent = false): Promise<void> {
		if (
			state.status === "restartFailed" ||
			(isBusyStatus(state.status) && state.status !== "checking")
		) {
			return Promise.resolve();
		}
		if (update) {
			presentationPending = incomingRequestActive;
			setState("dialogOpen", !incomingRequestActive);
			return Promise.resolve();
		}
		if (checkRequest) {
			if (!silent) manualCheckRequested = true;
			return checkRequest;
		}

		manualCheckRequested = !silent;
		setState({ status: "checking", errorCode: null, errorMessage: null });
		const request = (async () => {
			try {
				await loadCurrentVersion();
				const candidate = await dependencies.check();
				if (!candidate) {
					setState({
						availableVersion: null,
						dialogOpen: false,
						errorCode: null,
						errorMessage: null,
						releaseNotes: "",
						status: "upToDate",
					});
					return;
				}

				update = candidate;
				presentationPending = true;
				const showManualResult = manualCheckRequested && !incomingRequestActive;
				if (showManualResult) presentationPending = false;
				setState({
					availableVersion: candidate.version,
					currentVersion: candidate.currentVersion,
					dialogOpen: showManualResult,
					errorCode: null,
					errorMessage: null,
					progress: null,
					releaseNotes: candidate.body?.trim() ?? "",
					status: "available",
				});
			} catch (error) {
				if (!manualCheckRequested) {
					setState({ status: "idle", errorCode: null, errorMessage: null });
					return;
				}
				setState({
					status: "checkError",
					errorCode: isFeedUnavailable(error) ? "feedUnavailable" : "check",
					errorMessage: errorText(error),
				});
			} finally {
				manualCheckRequested = false;
			}
		})();
		checkRequest = request;
		void request.finally(() => {
			if (checkRequest === request) checkRequest = null;
		});
		return request;
	}

	function startupCheck(): Promise<void> {
		if (startupCheckStarted) return Promise.resolve();
		startupCheckStarted = true;
		if (dependencies.isDevelopment) return loadCurrentVersion();
		return checkForUpdates(true);
	}

	function presentDeferredUpdate() {
		if (!presentationPending || !update || state.dialogOpen || incomingRequestActive) return;
		presentationPending = false;
		setState("dialogOpen", true);
	}

	function setIncomingRequestActive(active: boolean) {
		const wasActive = incomingRequestActive;
		incomingRequestActive = active;
		if (active) {
			if (
				state.dialogOpen &&
				state.status !== "installing" &&
				state.status !== "restarting" &&
				state.status !== "restartFailed"
			) {
				presentationPending = true;
				setState("dialogOpen", false);
			}
			return;
		}
		if (wasActive) presentDeferredUpdate();
	}

	function dismissUpdate() {
		if (isBusyStatus(state.status) || state.status === "restartFailed") return;
		presentationPending = false;
		setState("dialogOpen", false);
	}

	function downloadUpdate(): Promise<void> {
		if (downloadRequest) return downloadRequest;
		if (!update || (state.status !== "available" && state.status !== "downloadError")) {
			return Promise.resolve();
		}

		presentationPending = incomingRequestActive;
		setState({
			dialogOpen: !incomingRequestActive,
			errorCode: null,
			errorMessage: null,
			progress: { downloaded: 0, percent: null, total: null },
			status: "downloading",
		});
		let downloaded = 0;
		let total: number | null = null;
		const selectedUpdate = update;
		const request = (async () => {
			try {
				await selectedUpdate.download((event) => {
					if (state.status !== "downloading") return;
					if (event.event === "Started") {
						const contentLength = event.data?.contentLength;
						if (typeof contentLength === "number" && contentLength > 0) {
							total = Math.max(total ?? 0, contentLength, downloaded);
						}
					} else if (event.event === "Progress") {
						downloaded += Math.max(0, event.data?.chunkLength ?? 0);
					} else if (event.event === "Finished" && total !== null) {
						downloaded = Math.max(downloaded, total);
					}
					setState("progress", {
						downloaded,
						percent: progressPercent(downloaded, total),
						total,
					});
				});
				if (total !== null) downloaded = Math.max(downloaded, total);
				setState("progress", {
					downloaded,
					percent: progressPercent(downloaded, total),
					total,
				});
				setState({ status: "downloaded", errorCode: null, errorMessage: null });
			} catch (error) {
				setState({
					status: "downloadError",
					errorCode: "download",
					errorMessage: errorText(error),
				});
			}
		})();
		downloadRequest = request;
		void request.finally(() => {
			if (downloadRequest === request) downloadRequest = null;
		});
		return request;
	}

	function installUpdate(): Promise<void> {
		if (installRequest || !update || state.status === "restartFailed") {
			return installRequest ?? Promise.resolve();
		}
		if (
			state.status !== "downloaded" &&
			state.status !== "waitingForTransfers" &&
			state.status !== "installError"
		) {
			return Promise.resolve();
		}

		const selectedUpdate = update;
		setState({ status: "preparingInstall", errorCode: null, errorMessage: null });
		const request = (async () => {
			setState({ status: "installing", errorCode: null, errorMessage: null });
			try {
				await selectedUpdate.install();
			} catch (error) {
				const failure = installFailure(error);
				setState({
					status: failure.code === "transferGate" ? "waitingForTransfers" : "installError",
					errorCode: failure.code,
					errorMessage: failure.message,
				});
				return;
			}

			setState({ status: "restarting", errorCode: null, errorMessage: null });
			try {
				await selectedUpdate.close();
			} catch {
				// The install succeeded; resource cleanup must not block restart.
			}
			try {
				await dependencies.relaunch();
			} catch (error) {
				setState({
					status: "restartFailed",
					errorCode: "restart",
					errorMessage: errorText(error),
				});
			}
		})();
		installRequest = request;
		void request.finally(() => {
			if (installRequest === request) installRequest = null;
		});
		return request;
	}

	return {
		state,
		checkForUpdates,
		dismissUpdate,
		downloadUpdate,
		isBusy,
		installUpdate,
		setIncomingRequestActive,
		presentDeferredUpdate,
		startupCheck,
	};
}
