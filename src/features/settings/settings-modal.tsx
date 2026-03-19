import { Show } from "solid-js";
import Icon from "~/components/icon";
import { Button } from "~/components/ui/button";
import { Tabs, TabsList, TabsTrigger } from "~/components/ui/tabs";
import { TextField, TextFieldInput } from "~/components/ui/text-field";

export type ThemeMode = "light" | "dark" | "system";

interface SettingsModalProps {
	open: boolean;
	destinationDir: string;
	theme: ThemeMode;
	onChangeDestination: () => void;
	onChangeTheme: (theme: ThemeMode) => void;
	onClose: () => void;
}

function SettingsModal(props: SettingsModalProps) {
	return (
		<Show when={props.open}>
			<div class="absolute inset-0 z-50 flex items-center justify-center">
				<div class="absolute inset-0 bg-background/60 backdrop-blur-[1px]" />
				<div class="relative z-10 mx-4 w-full max-w-xs space-y-4 rounded-lg border border-border bg-background p-5 shadow-lg">
					<div class="flex items-center justify-between">
						<h2 class="text-sm font-semibold">Settings</h2>
						<Button variant="ghost" size="icon" class="size-6" onClick={props.onClose}>
							<Icon name="lucide:x" size={14} />
							<span class="sr-only">Close</span>
						</Button>
					</div>
					<div class="space-y-3">
						<div class="space-y-1">
							<p class="text-xs font-medium text-muted-foreground">Save files to</p>
							<div class="flex items-center gap-1.5">
								<TextField class="min-w-0 flex-1">
									<TextFieldInput disabled value={props.destinationDir} class="bg-muted" />
								</TextField>
								<Button variant="outline" onClick={props.onChangeDestination}>
									Change
								</Button>
							</div>
						</div>
						<div class="space-y-1">
							<p class="text-xs font-medium text-muted-foreground">Appearance</p>
							<Tabs
								value={props.theme}
								onChange={(value) => props.onChangeTheme(value as ThemeMode)}
							>
								<TabsList class="h-8">
									<TabsTrigger value="light" class="capitalize">light</TabsTrigger>
									<TabsTrigger value="dark" class="capitalize">dark</TabsTrigger>
									<TabsTrigger value="system" class="capitalize">system</TabsTrigger>
								</TabsList>
							</Tabs>
						</div>
					</div>
					<div class="flex gap-2">
						<Button variant="outline" class="flex-1" onClick={props.onClose}>
							Cancel
						</Button>
						<Button class="flex-1" onClick={props.onClose}>
							Done
						</Button>
					</div>
				</div>
			</div>
		</Show>
	);
}

export default SettingsModal;
