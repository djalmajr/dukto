import { Button } from "~/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "~/components/ui/dialog";
import { ToggleGroup, ToggleGroupItem } from "~/components/ui/toggle-group";

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
		<Dialog open={props.open} onOpenChange={(open) => !open && props.onClose()}>
			<DialogContent class="max-w-xs">
				<DialogHeader>
					<DialogTitle>Settings</DialogTitle>
				</DialogHeader>
				<div class="space-y-4">
					<div class="space-y-1.5">
						<p class="text-xs font-medium text-muted-foreground">Save files to</p>
						<div class="flex items-center gap-2">
							<span class="min-w-0 flex-1 truncate rounded-md border border-input bg-muted px-2 py-1.5 text-xs">
								{props.destinationDir}
							</span>
							<Button variant="outline" size="sm" onClick={props.onChangeDestination}>
								Change
							</Button>
						</div>
					</div>
					<div class="space-y-1.5">
						<p class="text-xs font-medium text-muted-foreground">Appearance</p>
						<ToggleGroup
							value={props.theme}
							onChange={(value) => {
								if (value) props.onChangeTheme(value as ThemeMode);
							}}
							class="w-full"
							variant="outline"
							size="sm"
						>
							<ToggleGroupItem value="light" class="flex-1 capitalize">
								light
							</ToggleGroupItem>
							<ToggleGroupItem value="dark" class="flex-1 capitalize">
								dark
							</ToggleGroupItem>
							<ToggleGroupItem value="system" class="flex-1 capitalize">
								system
							</ToggleGroupItem>
						</ToggleGroup>
					</div>
				</div>
				<DialogFooter>
					<Button class="w-full" onClick={props.onClose}>
						Done
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}

export default SettingsModal;
