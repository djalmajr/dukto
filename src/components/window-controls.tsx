import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "~/components/ui/button";
import { t } from "~/helpers/i18n";
import CarbonClose from "~icons/carbon/close";
import CarbonMaximize from "~icons/carbon/maximize";
import CarbonSubtract from "~icons/carbon/subtract";

export default function WindowControls() {
	return (
		<div class="ml-auto flex h-full shrink-0" data-no-window-drag>
			<Button
				variant="ghost"
				class="h-full w-11 rounded-none"
				aria-label={t("minimizeWindow")}
				title={t("minimizeWindow")}
				onClick={() => void getCurrentWindow().minimize()}
			>
				<CarbonSubtract class="size-4" />
			</Button>
			<Button
				variant="ghost"
				class="h-full w-11 rounded-none"
				disabled
				aria-label={t("maximizeWindow")}
				title={t("maximizeWindow")}
			>
				<CarbonMaximize class="size-4" />
			</Button>
			<Button
				variant="ghost"
				class="h-full w-11 rounded-none hover:bg-destructive hover:text-destructive-foreground"
				aria-label={t("closeWindow")}
				title={t("closeWindow")}
				onClick={() => void getCurrentWindow().close()}
			>
				<CarbonClose class="size-4" />
			</Button>
		</div>
	);
}
