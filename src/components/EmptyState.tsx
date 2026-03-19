interface EmptyStateProps {
	title: string;
	description?: string;
}

function EmptyState(props: EmptyStateProps) {
	return (
		<div class="flex flex-col items-center justify-center py-12 text-center">
			<div class="text-3xl text-zinc-300 dark:text-zinc-600">📡</div>
			<p class="mt-3 text-sm font-medium text-zinc-500">{props.title}</p>
			{props.description && <p class="mt-1 text-xs text-zinc-400">{props.description}</p>}
		</div>
	);
}

export default EmptyState;
