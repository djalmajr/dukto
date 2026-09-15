import { Show, createEffect, createSignal } from "solid-js";
import { Button } from "~/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "~/components/ui/dialog";
import { t } from "~/helpers/i18n";
import { nativeErrorKey } from "~/helpers/native-error";
import { formatBytes } from "~/utils/format";

export interface IncomingRequestData {
	sender_name: string;
	item_count: number;
	total_size: number;
}

interface IncomingRequestProps {
	request: IncomingRequestData | null;
	onAccept: () => void | Promise<void>;
	onReject: () => void | Promise<void>;
}

function IncomingRequestDialog(props: IncomingRequestProps) {
	const [busy, setBusy] = createSignal(false);
	const [responseError, setResponseError] = createSignal<string | null>(null);
	createEffect(() => {
		props.request;
		setResponseError(null);
	});

	async function respond(action: IncomingRequestProps["onAccept"]) {
		if (busy()) return;
		setBusy(true);
		setResponseError(null);
		try {
			await action();
		} catch (error) {
			setResponseError(String(error));
		} finally {
			setBusy(false);
		}
	}

	return (
		<Show when={props.request}>
			{(req) => (
				<Dialog open onOpenChange={(open) => !open && void respond(props.onReject)}>
					<DialogContent
						class="max-w-xs"
						onInteractOutside={(e: Event) => e.preventDefault()}
						onEscapeKeyDown={(e: Event) => e.preventDefault()}
					>
						<DialogHeader>
							<DialogTitle>{t("incoming")}</DialogTitle>
							<DialogDescription>
								{t("items", { count: req().item_count })} &middot; {formatBytes(req().total_size)}
								<br />
								{t("senderLabel")} <span class="break-all">{req().sender_name}</span>
							</DialogDescription>
						</DialogHeader>
						<Show when={responseError()}>
							{(message) => <p class="text-sm text-destructive">{t(nativeErrorKey(message()))}</p>}
						</Show>
						<DialogFooter class="flex-row gap-2">
							<Button class="flex-1" disabled={busy()} onClick={() => void respond(props.onAccept)}>
								{t("accept")}
							</Button>
							<Button
								variant="outline"
								class="flex-1"
								disabled={busy()}
								onClick={() => void respond(props.onReject)}
							>
								{t("reject")}
							</Button>
						</DialogFooter>
					</DialogContent>
				</Dialog>
			)}
		</Show>
	);
}

export default IncomingRequestDialog;
