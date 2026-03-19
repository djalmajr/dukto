import { For, type JSX, createEffect, createMemo, createSignal } from "solid-js";
import Icon from "~/components/icon";
import { resolvedTheme } from "../stores/app";

const FIGMA_FILE_KEY = "U4hNHfRc8UGfcQk0GEEh8s";
const FIGMA_CAPTURE_URL = `#figmacapture=${FIGMA_FILE_KEY}&figmaendpoint=https://mcp.figma.com&figmaselector=*`;

interface Preset {
	id: string;
	label: string;
	width: number;
	height: number;
}

interface ViewportState {
	width: number;
	height: number;
	zoom: number;
	activePresetId: string | null;
	mode: "preset" | "manual" | "drag";
}

interface DragSession {
	edge: ResizeEdge;
	pointerId: number;
	startX: number;
	startY: number;
	startWidth: number;
	startHeight: number;
}

type ResizeEdge = "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw";

const PRESETS: Preset[] = [
	{ id: "mobile-s", label: "Mobile S", width: 320, height: 568 },
	{ id: "iphone-se", label: "iPhone SE", width: 375, height: 667 },
	{ id: "iphone-xr", label: "iPhone XR", width: 414, height: 896 },
	{ id: "iphone-12-pro", label: "iPhone 12 Pro", width: 390, height: 844 },
	{ id: "iphone-14", label: "iPhone 14", width: 393, height: 852 },
	{ id: "iphone-14-pro-max", label: "iPhone 14 Pro Max", width: 430, height: 932 },
	{ id: "pixel-7", label: "Pixel 7", width: 412, height: 915 },
	{ id: "galaxy-s8-plus", label: "Galaxy S8+", width: 360, height: 740 },
	{ id: "galaxy-s20-ultra", label: "Galaxy S20 Ultra", width: 412, height: 915 },
	{ id: "desktop-app", label: "Desktop App", width: 480, height: 640 },
	{ id: "ipad-mini", label: "iPad Mini", width: 768, height: 1024 },
	{ id: "ipad-air", label: "iPad Air", width: 820, height: 1180 },
	{ id: "ipad-pro", label: "iPad Pro", width: 1024, height: 1366 },
	{ id: "surface-pro-7", label: "Surface Pro 7", width: 912, height: 1368 },
	{ id: "laptop", label: "Laptop", width: 1024, height: 768 },
	{ id: "desktop", label: "Desktop", width: 1280, height: 800 },
	{ id: "nest-hub", label: "Nest Hub", width: 1024, height: 600 },
];

const ZOOMS = [0.5, 0.75, 0.92, 1, 1.25];
const DEFAULT_WIDTH = 600;
const DEFAULT_HEIGHT = 640;
const MIN_WIDTH = 320;
const MIN_HEIGHT = 400;

interface WindowFrameProps {
	children: JSX.Element;
	overlay?: JSX.Element;
	toolbar?: JSX.Element;
	deviceName?: string;
	hostname?: string;
	ref?: (el: HTMLDivElement) => void;
	onSettingsClick?: () => void;
}

export { type Preset, PRESETS };

function WindowFrame(props: WindowFrameProps) {
	const isDark = () => resolvedTheme() === "dark";
	const [viewport, setViewport] = createSignal<ViewportState>({
		width: DEFAULT_WIDTH,
		height: DEFAULT_HEIGHT,
		zoom: 1,
		activePresetId: null,
		mode: "manual",
	});
	const [dragSession, setDragSession] = createSignal<DragSession | null>(null);

	const scaledWidth = createMemo(() => Math.round(viewport().width * viewport().zoom));
	const scaledHeight = createMemo(() => Math.round(viewport().height * viewport().zoom));

	function findMatchingPreset(width: number, height: number) {
		return PRESETS.find((preset) => preset.width === width && preset.height === height) ?? null;
	}

	function getMaxDimensions() {
		const stagePadding = 64;
		const width = Math.max(MIN_WIDTH, window.innerWidth - stagePadding * 2);
		const height = Math.max(MIN_HEIGHT, window.innerHeight - 120);
		return { width, height };
	}

	function clampDimensions(width: number, height: number) {
		const max = getMaxDimensions();
		return {
			width: Math.min(Math.max(Math.round(width), MIN_WIDTH), max.width),
			height: Math.min(Math.max(Math.round(height), MIN_HEIGHT), max.height),
		};
	}

	function applyDimensions(width: number, height: number, mode: ViewportState["mode"]) {
		const next = clampDimensions(width, height);
		const matchingPreset = findMatchingPreset(next.width, next.height);
		setViewport((current) => ({
			...current,
			width: next.width,
			height: next.height,
			activePresetId: matchingPreset?.id ?? null,
			mode,
		}));
	}

	function selectPreset(preset: Preset) {
		setViewport((current) => ({
			...current,
			width: preset.width,
			height: preset.height,
			activePresetId: preset.id,
			mode: "preset",
		}));
	}

	function handlePresetSelect(value: string) {
		if (value === "responsive") {
			setViewport((current) => ({
				...current,
				activePresetId: null,
				mode: "manual",
			}));
			return;
		}

		const preset = PRESETS.find((item) => item.id === value);
		if (preset) selectPreset(preset);
	}

	function handleWidthInput(value: string) {
		const parsed = Number.parseInt(value, 10);
		if (!Number.isNaN(parsed) && parsed > 0) {
			applyDimensions(parsed, viewport().height, "manual");
		}
	}

	function handleHeightInput(value: string) {
		const parsed = Number.parseInt(value, 10);
		if (!Number.isNaN(parsed) && parsed > 0) {
			applyDimensions(viewport().width, parsed, "manual");
		}
	}

	function setZoom(value: number) {
		setViewport((current) => ({
			...current,
			zoom: value,
		}));
	}

	function finishDrag() {
		setDragSession(null);
		setViewport((current) => ({
			...current,
			mode: current.activePresetId ? "preset" : "manual",
		}));
		window.removeEventListener("pointermove", handlePointerMove);
		window.removeEventListener("pointerup", handlePointerEnd);
		window.removeEventListener("pointercancel", handlePointerEnd);
		document.body.style.userSelect = "";
	}

	function handlePointerMove(event: PointerEvent) {
		const session = dragSession();
		if (!session || event.pointerId !== session.pointerId) return;

		const zoom = viewport().zoom || 1;
		const deltaX = (event.clientX - session.startX) / zoom;
		const deltaY = (event.clientY - session.startY) / zoom;

		let nextWidth = session.startWidth;
		let nextHeight = session.startHeight;

		if (session.edge.includes("e")) nextWidth = session.startWidth + deltaX;
		if (session.edge.includes("w")) nextWidth = session.startWidth - deltaX;
		if (session.edge.includes("s")) nextHeight = session.startHeight + deltaY;
		if (session.edge.includes("n")) nextHeight = session.startHeight - deltaY;

		applyDimensions(nextWidth, nextHeight, "drag");
	}

	function handlePointerEnd(event: PointerEvent) {
		if (dragSession()?.pointerId !== event.pointerId) return;
		finishDrag();
	}

	function startResize(edge: ResizeEdge, event: PointerEvent & { currentTarget: HTMLElement }) {
		event.preventDefault();
		event.currentTarget.setPointerCapture(event.pointerId);
		setDragSession({
			edge,
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
			startWidth: viewport().width,
			startHeight: viewport().height,
		});
		setViewport((current) => ({
			...current,
			mode: "drag",
			activePresetId: null,
		}));
		document.body.style.userSelect = "none";
		window.addEventListener("pointermove", handlePointerMove);
		window.addEventListener("pointerup", handlePointerEnd);
		window.addEventListener("pointercancel", handlePointerEnd);
	}

	const handleBase = "absolute z-20 touch-none select-none bg-transparent";

	createEffect(() => {
		const dark = isDark();
		document.documentElement.classList.toggle("dark", dark);
		document.documentElement.setAttribute("data-kb-theme", dark ? "dark" : "light");
	});

	return (
		<div
			class="flex min-h-screen flex-col items-center bg-muted"
			classList={{ dark: isDark() }}
		>
			<div class="sticky top-0 z-30 flex w-full flex-wrap items-center justify-center gap-1.5 border-b border-border bg-background px-2 py-1.5 text-xs text-muted-foreground">
				{props.toolbar}
				<select
					class="h-6 rounded border border-border bg-background px-1.5 text-xs outline-none focus:border-primary"
					value={viewport().activePresetId ?? "responsive"}
					onChange={(event) => handlePresetSelect(event.currentTarget.value)}
				>
					<option value="responsive">Responsive</option>
					<For each={PRESETS}>{(preset) => <option value={preset.id}>{preset.label}</option>}</For>
				</select>
				<div class="flex items-center gap-0.5">
					<input
						type="number"
						class="h-6 w-14 appearance-none rounded border border-border bg-background px-1 text-center text-xs tabular-nums outline-none focus:border-primary [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
						value={viewport().width}
						onChange={(event) => handleWidthInput(event.currentTarget.value)}
					/>
					<span class="text-muted-foreground/40">&times;</span>
					<input
						type="number"
						class="h-6 w-14 appearance-none rounded border border-border bg-background px-1 text-center text-xs tabular-nums outline-none focus:border-primary [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
						value={viewport().height}
						onChange={(event) => handleHeightInput(event.currentTarget.value)}
					/>
				</div>
				<select
					class="h-6 rounded border border-border bg-background px-1.5 text-xs outline-none focus:border-primary"
					value={String(viewport().zoom)}
					onChange={(event) => setZoom(Number(event.currentTarget.value))}
				>
					<For each={ZOOMS}>
						{(zoom) => <option value={String(zoom)}>{Math.round(zoom * 100)}%</option>}
					</For>
				</select>
				<a
					href={FIGMA_CAPTURE_URL}
					class="ml-1 flex h-6 items-center gap-1 rounded border border-border px-1.5 text-xs text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
					title="Send to Figma"
				>
					<Icon name="lucide:figma" size={12} />
					Figma
				</a>
			</div>
			<div class="flex flex-1 items-center justify-center p-8">
				<div class="flex flex-col items-center">
					<div
						class="relative"
						style={{ width: `${scaledWidth()}px`, height: `${scaledHeight()}px` }}
					>
						<div
							class="absolute inset-0"
							style={{
								transform: `scale(${viewport().zoom})`,
								"transform-origin": "top center",
							}}
						>
							<div
								ref={props.ref}
								class="relative flex h-full flex-col overflow-hidden rounded-xl border border-border bg-background shadow-2xl"
								classList={{ dark: isDark() }}
								style={{
									width: `${viewport().width}px`,
									height: `${viewport().height}px`,
									"transition-duration": dragSession() ? "0ms" : "200ms",
								}}
							>
								<div class="flex shrink-0 items-center gap-2 border-b border-border bg-muted px-4 py-2.5">
									<div class="flex gap-1.5">
										<div class="h-3 w-3 rounded-full bg-red-400" />
										<div class="h-3 w-3 rounded-full bg-yellow-400" />
										<div class="h-3 w-3 rounded-full bg-green-400" />
									</div>
									<div class="flex-1 text-center">
										{(props.deviceName || props.hostname) && (
											<p class="text-xs font-medium text-muted-foreground">
												{props.deviceName}
												{props.deviceName && props.hostname && " . "}
												{props.hostname}
											</p>
										)}
									</div>
									<div class="flex items-center gap-1">
										{props.onSettingsClick && (
											<button
												type="button"
												class="flex h-6 w-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
												onClick={props.onSettingsClick}
												title="Settings"
											>
												<Icon name="lucide:settings" size={16} />
											</button>
										)}
									</div>
								</div>
								<div class="relative flex-1 overflow-hidden bg-background text-foreground">
									<div class="absolute inset-0 overflow-y-auto">
										<div class="flex min-h-full flex-col items-center px-4 py-6">
											{props.children}
										</div>
									</div>
									{props.overlay}
								</div>
							</div>

							<div
								class={`${handleBase} -top-2 left-2 right-2 h-4 cursor-ns-resize`}
								onPointerDown={(event) => startResize("n", event)}
							/>
							<div
								class={`${handleBase} -bottom-2 left-2 right-2 h-4 cursor-ns-resize`}
								onPointerDown={(event) => startResize("s", event)}
							/>
							<div
								class={`${handleBase} -left-2 bottom-2 top-2 w-4 cursor-ew-resize`}
								onPointerDown={(event) => startResize("w", event)}
							/>
							<div
								class={`${handleBase} -right-2 bottom-2 top-2 w-4 cursor-ew-resize`}
								onPointerDown={(event) => startResize("e", event)}
							/>
							<div
								class={`${handleBase} -left-2 -top-2 h-5 w-5 cursor-nwse-resize`}
								onPointerDown={(event) => startResize("nw", event)}
							/>
							<div
								class={`${handleBase} -right-2 -top-2 h-5 w-5 cursor-nesw-resize`}
								onPointerDown={(event) => startResize("ne", event)}
							/>
							<div
								class={`${handleBase} -left-2 -bottom-2 h-5 w-5 cursor-nesw-resize`}
								onPointerDown={(event) => startResize("sw", event)}
							/>
							<div
								class={`${handleBase} -bottom-2 -right-2 h-5 w-5 cursor-nwse-resize`}
								onPointerDown={(event) => startResize("se", event)}
							/>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
}

export default WindowFrame;
