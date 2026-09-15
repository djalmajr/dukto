import { Show } from "solid-js";
import { Button } from "~/components/ui/button";
import { Dialog, DialogContent, DialogTitle } from "~/components/ui/dialog";
import { Tabs, TabsList, TabsTrigger } from "~/components/ui/tabs";
import { TextField, TextFieldInput } from "~/components/ui/text-field";
import { t } from "~/helpers/i18n";

export type ThemeMode = "light" | "dark" | "system";

interface SettingsModalProps {
	open: boolean;
	destinationDir: string;
	theme: ThemeMode;
	language?: string;
	onChangeDestination: () => void;
	onChangeTheme: (theme: ThemeMode) => void;
	onChangeLanguage?: (lang: string) => void;
	onClose: () => void;
}

function SettingsModal(props: SettingsModalProps) {
	return (
		<Dialog open={props.open} onOpenChange={(open) => !open && props.onClose()}>
			<DialogContent
				class="max-h-[calc(100dvh-2rem)] w-[calc(100%-2rem)] max-w-xs gap-4 rounded-lg p-5"
				overlayClass="bg-black/40"
				onEscapeKeyDown={(event: Event) => event.preventDefault()}
			>
				<DialogTitle class="text-sm font-semibold">{t("settings")}</DialogTitle>
				<div class="space-y-3">
					<div class="space-y-1">
						<p class="text-xs font-medium text-muted-foreground">{t("saveFilesTo")}</p>
						<div class="flex items-center gap-1.5">
							<TextField class="min-w-0 flex-1">
								<TextFieldInput disabled value={props.destinationDir} class="bg-muted" />
							</TextField>
							<Button variant="outline" onClick={props.onChangeDestination}>
								{t("change")}
							</Button>
						</div>
					</div>
					<div class="space-y-1">
						<p class="text-xs font-medium text-muted-foreground">{t("appearance")}</p>
						<Tabs value={props.theme} onChange={(value) => props.onChangeTheme(value as ThemeMode)}>
							<TabsList class="h-8">
								<TabsTrigger value="light" class="capitalize">
									{t("light")}
								</TabsTrigger>
								<TabsTrigger value="dark" class="capitalize">
									{t("dark")}
								</TabsTrigger>
								<TabsTrigger value="system" class="capitalize">
									{t("system")}
								</TabsTrigger>
							</TabsList>
						</Tabs>
					</div>
					<Show when={props.onChangeLanguage}>
						<div class="space-y-1">
							<p class="text-xs font-medium text-muted-foreground">{t("language")}</p>
							<Tabs
								value={props.language ?? "pt"}
								onChange={(value) => props.onChangeLanguage?.(value)}
							>
								<TabsList class="h-8">
									<TabsTrigger value="en">English</TabsTrigger>
									<TabsTrigger value="pt">Português</TabsTrigger>
									<TabsTrigger value="es">Español</TabsTrigger>
								</TabsList>
							</Tabs>
						</div>
					</Show>
				</div>
			</DialogContent>
		</Dialog>
	);
}

export default SettingsModal;
