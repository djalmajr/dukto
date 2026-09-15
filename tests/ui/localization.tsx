import { render } from "solid-js/web";
import { Button } from "../../src/components/ui/button";
import { changeLanguage } from "../../src/helpers/i18n";
import PeerCard from "../../src/routes/-components/peers/peer-card";
import SendPreview from "../../src/routes/-components/transfers/send-preview";
import "../../src/styles/index.css";

changeLanguage("pt");
const peer = { device_id: "linux", display_name: "ubuntu", hostname: "lab", platform: "linux" };
function Fixture() {
	return (
		<main class="mx-auto max-w-lg space-y-4 p-6">
			<div class="flex gap-2">
				<Button onClick={() => changeLanguage("pt")}>Português</Button>
				<Button onClick={() => changeLanguage("es")}>Español</Button>
				<Button onClick={() => changeLanguage("en")}>English</Button>
			</div>
			<PeerCard
				peer={peer}
				onAddItems={() => {}}
				transfers={[
					{
						id: "error",
						direction: "receive",
						status: "error",
						percent: 30,
						bytesSent: 30,
						bytesTotal: 100,
						errorMsg: "connection lost",
					},
				]}
			/>
			<SendPreview
				peer={peer}
				files={[
					{ name: "a.txt", path: "/test/a.txt", size: 12, is_dir: false },
					{ name: "b.txt", path: "/test/b.txt", size: 12, is_dir: false },
				]}
				onConfirm={() => {}}
				onCancel={() => {}}
				onRemoveFile={() => {}}
			/>
		</main>
	);
}
const root = document.getElementById("root");
if (!root) throw new Error("Missing fixture root");
render(() => <Fixture />, root);
