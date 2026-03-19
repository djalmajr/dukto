import { Show } from "solid-js";
import Button from "../../components/Button";
import Modal from "../../components/Modal";
import { formatBytes } from "../../lib/format";

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
				<Modal>
					<h3 class="text-sm font-semibold">Incoming transfer</h3>
					<p class="mt-2 text-xs text-zinc-500">
						{req().item_count} {req().item_count === 1 ? "item" : "items"} &middot;{" "}
						{formatBytes(req().total_size)}
					</p>
					<p class="mt-1 text-xs text-zinc-400">From: {req().sender_name}</p>
					<div class="mt-3 flex gap-2">
						<Button variant="primary" class="flex-1" onClick={props.onAccept}>
							Accept
						</Button>
						<Button class="flex-1" onClick={props.onReject}>
							Reject
						</Button>
					</div>
				</Modal>
			)}
		</Show>
	);
}

export default IncomingRequestDialog;
