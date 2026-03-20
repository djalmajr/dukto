import { Show } from "solid-js";
import { t } from "~/lib/i18n";
import { Button } from "~/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "~/components/ui/dialog";
import { formatBytes } from "~/lib/format";

export interface IncomingRequestData {
	sender_name: string;
	item_count: number;
	total_size: number;
}

interface IncomingRequestProps {
	request: IncomingRequestData | null;
	onAccept: () => void;
	onReject: () => void;
}

function IncomingRequestDialog(props: IncomingRequestProps) {
	return (
		<Show when={props.request}>
			{(req) => (
				<Dialog open onOpenChange={(open) => !open && props.onReject()}>
					<DialogContent
						class="max-w-xs"
						onInteractOutside={(e: Event) => e.preventDefault()}
						onEscapeKeyDown={(e: Event) => e.preventDefault()}
					>
						<DialogHeader>
							<DialogTitle>{t("incoming")}</DialogTitle>
							<DialogDescription>
								{req().item_count} {req().item_count === 1 ? "item" : "items"} &middot;{" "}
								{formatBytes(req().total_size)}
								<br />
								From: {req().sender_name}
							</DialogDescription>
						</DialogHeader>
						<DialogFooter class="flex-row gap-2">
							<Button class="flex-1" onClick={props.onAccept}>
								{t("accept")}
							</Button>
							<Button variant="outline" class="flex-1" onClick={props.onReject}>
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
