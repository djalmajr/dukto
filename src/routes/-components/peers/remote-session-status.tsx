import { createSignal, onCleanup, onMount } from "solid-js";
import { t } from "~/helpers/i18n";

type RemoteRoute = "direct" | "relay";

export interface RemoteStatusValue {
	state: string;
	route?: RemoteRoute;
}

export interface RemoteStatusDescription {
	labelKey: string;
	tone: "info" | "success" | "muted" | "error";
}

export function describeRemoteSession(
	status: RemoteStatusValue,
	expiresAtUnix: number,
	nowUnix: number,
): RemoteStatusDescription {
	if (
		nowUnix >= expiresAtUnix &&
		!(["completed", "cancelled", "expired", "failed"] as string[]).includes(status.state)
	) {
		return { labelKey: "remoteStatusExpired", tone: "muted" };
	}

	switch (status.state) {
		case "connecting":
			return { labelKey: "remoteStatusConnecting", tone: "info" };
		case "invited":
			return { labelKey: "remoteStatusWaiting", tone: "info" };
		case "pairing":
			return { labelKey: "remoteStatusPairing", tone: "info" };
		case "ready":
			return status.route === "relay"
				? { labelKey: "remoteStatusReadyRelay", tone: "success" }
				: { labelKey: "remoteStatusReadyDirect", tone: "success" };
		case "transferring":
			return status.route === "relay"
				? { labelKey: "remoteStatusTransferringRelay", tone: "success" }
				: { labelKey: "remoteStatusTransferringDirect", tone: "success" };
		case "completed":
			return { labelKey: "remoteStatusCompleted", tone: "success" };
		case "cancelled":
			return { labelKey: "remoteStatusCancelled", tone: "muted" };
		case "expired":
			return { labelKey: "remoteStatusExpired", tone: "muted" };
		default:
			return { labelKey: "remoteStatusFailed", tone: "error" };
	}
}

interface RemoteSessionStatusProps {
	status: RemoteStatusValue;
	expiresAtUnix: number;
	error?: string | null;
}

function RemoteSessionStatus(props: RemoteSessionStatusProps) {
	const [now, setNow] = createSignal(Math.floor(Date.now() / 1000));
	let timer: number | undefined;
	onMount(() => {
		timer = window.setInterval(() => setNow(Math.floor(Date.now() / 1000)), 1000);
	});
	onCleanup(() => window.clearInterval(timer));

	const description = () =>
		props.error
			? ({ labelKey: props.error, tone: "error" } as const)
			: describeRemoteSession(props.status, props.expiresAtUnix, now());

	return (
		<div
			role={description().tone === "error" ? "alert" : "status"}
			aria-live="polite"
			class="flex min-h-6 items-center gap-2 text-xs"
			classList={{
				"text-muted-foreground": description().tone === "muted",
				"text-foreground": description().tone === "info",
				"text-success-foreground": description().tone === "success",
				"text-error-foreground": description().tone === "error",
			}}
		>
			<span aria-hidden="true" class="size-1.5 shrink-0 rounded-full bg-current" />
			<span>{props.error ? description().labelKey : t(description().labelKey)}</span>
		</div>
	);
}

export default RemoteSessionStatus;
