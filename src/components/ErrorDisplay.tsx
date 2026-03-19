interface ErrorDisplayProps {
	message: string;
	onDismiss?: () => void;
}

function ErrorDisplay(props: ErrorDisplayProps) {
	return (
		<div class="flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 p-3 dark:border-red-900 dark:bg-red-950/30">
			<span class="shrink-0 text-sm">⚠️</span>
			<div class="min-w-0 flex-1">
				<p class="text-xs text-red-700 dark:text-red-400">{props.message}</p>
			</div>
			{props.onDismiss && (
				<button
					type="button"
					class="shrink-0 text-xs text-red-400 hover:text-red-600 dark:hover:text-red-300"
					onClick={props.onDismiss}
				>
					✕
				</button>
			)}
		</div>
	);
}

export default ErrorDisplay;
