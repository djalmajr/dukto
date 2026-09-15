import { Button } from "~/components/ui/button";
import { t } from "~/helpers/i18n";
interface ErrorDisplayProps {
	message: string;
	onDismiss?: () => void;
}

function ErrorDisplay(props: ErrorDisplayProps) {
	return (
		<div class="flex items-start gap-2.5 rounded-lg border border-error bg-error/50 p-3">
			<svg
				aria-hidden="true"
				class="mt-0.5 h-4 w-4 shrink-0 text-error-foreground"
				fill="none"
				stroke="currentColor"
				stroke-width="2"
				viewBox="0 0 24 24"
			>
				<path
					stroke-linecap="round"
					stroke-linejoin="round"
					d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z"
				/>
			</svg>
			<div class="min-w-0 flex-1">
				<p class="text-xs text-error-foreground">{props.message}</p>
			</div>
			{props.onDismiss && (
				<Button
					variant="ghost"
					size="icon-sm"
					aria-label={t("dismissError")}
					class="shrink-0 rounded p-0.5 text-error-foreground/60 transition-colors hover:text-error-foreground"
					onClick={props.onDismiss}
				>
					<svg
						aria-hidden="true"
						class="h-3.5 w-3.5"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						viewBox="0 0 24 24"
					>
						<path stroke-linecap="round" stroke-linejoin="round" d="M6 18 18 6M6 6l12 12" />
					</svg>
				</Button>
			)}
		</div>
	);
}

export default ErrorDisplay;
