import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "~/components/ui/button";
import { t } from "~/helpers/i18n";
import Minus from "~icons/lucide/minus";
import Square from "~icons/lucide/square";
import X from "~icons/lucide/x";

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
				<Minus class="size-3" />
			</Button>
			<Button
				variant="ghost"
				class="h-full w-11 rounded-none"
				disabled
				aria-label={t("maximizeWindow")}
				title={t("maximizeWindow")}
			>
				<Square class="size-3" />
			</Button>
			<Button
				variant="ghost"
				class="h-full w-11 rounded-none hover:bg-destructive hover:text-destructive-foreground"
				aria-label={t("closeWindow")}
				title={t("closeWindow")}
				onClick={() => void getCurrentWindow().close()}
			>
				<X class="size-3" />
			</Button>
		</div>
	);
}
