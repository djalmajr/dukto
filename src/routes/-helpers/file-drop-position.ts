interface DropPosition {
	x: number;
	y: number;
}

export function fileDropPosition(
	position: DropPosition,
	platform: string,
	pixelRatio: number,
): DropPosition {
	// Wry 0.54.4's WKWebView backend forwards NSDraggingInfo coordinates in
	// logical points, although Tauri wraps them as PhysicalPosition. Converting
	// those points again on Retina screens selects the host above the cursor.
	const scale = platform.startsWith("Mac") ? 1 : pixelRatio;
	return { x: position.x / scale, y: position.y / scale };
}
