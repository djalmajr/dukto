export function orderedPeerIds(available: string[], saved: string[]): string[] {
	const present = new Set(available);
	return [...new Set([...saved.filter((id) => present.has(id)), ...available])];
}

// Keep absent hosts in the preference so they return to their chosen position.
export function movePeer(
	saved: string[],
	visible: string[],
	source: string,
	target: string,
	after: boolean,
): string[] {
	const order = [...new Set([...saved, ...visible])];
	if (source === target || !visible.includes(source) || !visible.includes(target)) return order;
	const next = order.filter((id) => id !== source);
	next.splice(next.indexOf(target) + Number(after), 0, source);
	return next;
}
