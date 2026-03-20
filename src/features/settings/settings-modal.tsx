import { Show } from "solid-js";
import { t } from "~/lib/i18n";
import Icon from "~/components/icon";
import { Button } from "~/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "~/components/ui/tabs";
import { TextField, TextFieldInput } from "~/components/ui/text-field";

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
		<Show when={props.open}>
			<div class="absolute inset-0 z-50 flex items-center justify-center">
				<div class="absolute inset-0 bg-background/60 backdrop-blur-[1px]" />
				<div class="relative z-10 mx-4 w-full max-w-xs space-y-4 rounded-lg border border-border bg-background p-5 shadow-lg">
					<div class="flex items-center justify-between">
						<h2 class="text-sm font-semibold">{t("settings")}</h2>
						<Button variant="ghost" size="icon" class="size-6" onClick={props.onClose}>
							<Icon name="lucide:x" size={14} />
							<span class="sr-only">Close</span>
						</Button>
					</div>
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
							<Tabs
								value={props.theme}
								onChange={(value) => props.onChangeTheme(value as ThemeMode)}
							>
								<TabsList class="h-8">
									<TabsTrigger value="light" class="capitalize">{t("light")}</TabsTrigger>
									<TabsTrigger value="dark" class="capitalize">{t("dark")}</TabsTrigger>
									<TabsTrigger value="system" class="capitalize">{t("system")}</TabsTrigger>
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
				</div>
			</div>
		</Show>
	);
}

export default SettingsModal;
