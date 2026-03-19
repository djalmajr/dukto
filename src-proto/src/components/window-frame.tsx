import { For, type JSX, createEffect, createMemo, createSignal } from "solid-js";
import { resolvedTheme } from "../stores/app";

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
const DEFAULT_PRESET = PRESETS[2];
const MIN_WIDTH = 320;
const MIN_HEIGHT = 400;

interface WindowFrameProps {
	children: JSX.Element;
	overlay?: JSX.Element;
	toolbar?: JSX.Element;
	deviceName?: string;
	hostname?: string;
	onSettingsClick?: () => void;
}

export { type Preset, PRESETS };

function WindowFrame(props: WindowFrameProps) {
	const isDark = () => resolvedTheme() === "dark";
	const [viewport, setViewport] = createSignal<ViewportState>({
		width: DEFAULT_PRESET.width,
		height: DEFAULT_PRESET.height,
		zoom: 1,
		activePresetId: DEFAULT_PRESET.id,
		mode: "preset",
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
			class="flex min-h-screen flex-col items-center bg-muted p-8"
			classList={{ dark: isDark() }}
		>
			<div class="mb-3 flex flex-wrap items-center justify-center gap-2 rounded-md border border-border bg-background px-3 py-1.5 text-xs text-muted-foreground shadow-sm">
				{props.toolbar}
				<span class="text-muted-foreground/40">|</span>
				<select
					class="h-8 rounded-md border border-border bg-background px-2 text-xs outline-none focus:border-primary"
					value={viewport().activePresetId ?? "responsive"}
					onChange={(event) => handlePresetSelect(event.currentTarget.value)}
				>
					<option value="responsive">Responsive</option>
					<For each={PRESETS}>{(preset) => <option value={preset.id}>{preset.label}</option>}</For>
				</select>
				<div class="flex items-center gap-1">
					<input
						type="number"
						class="h-8 w-18 appearance-none rounded-md border border-border bg-background px-2 text-center text-xs tabular-nums outline-none focus:border-primary [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
						value={viewport().width}
						onInput={(event) => handleWidthInput(event.currentTarget.value)}
					/>
					<span class="text-muted-foreground/60">&times;</span>
					<input
						type="number"
						class="h-8 w-18 appearance-none rounded-md border border-border bg-background px-2 text-center text-xs tabular-nums outline-none focus:border-primary [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none"
						value={viewport().height}
						onInput={(event) => handleHeightInput(event.currentTarget.value)}
					/>
				</div>
				<select
					class="h-8 rounded-md border border-border bg-background px-2 text-xs outline-none focus:border-primary"
					value={String(viewport().zoom)}
					onChange={(event) => setZoom(Number(event.currentTarget.value))}
				>
					<For each={ZOOMS}>
						{(zoom) => <option value={String(zoom)}>{Math.round(zoom * 100)}%</option>}
					</For>
				</select>
			</div>
			<div class="flex items-center justify-center">
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
												<svg
													class="h-4 w-4"
													fill="none"
													stroke="currentColor"
													stroke-width="2"
													viewBox="0 0 24 24"
													aria-hidden="true"
												>
													<path
														stroke-linecap="round"
														stroke-linejoin="round"
														d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.325.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 0 1 1.37.49l1.296 2.247a1.125 1.125 0 0 1-.26 1.431l-1.003.827c-.293.241-.438.613-.43.992a7.723 7.723 0 0 1 0 .255c-.008.378.137.75.43.991l1.004.827c.424.35.534.955.26 1.43l-1.298 2.247a1.125 1.125 0 0 1-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.47 6.47 0 0 1-.22.128c-.331.183-.581.495-.644.869l-.213 1.281c-.09.543-.56.94-1.11.94h-2.594c-.55 0-1.019-.398-1.11-.94l-.212-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 0 1-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 0 1-1.369-.49l-1.297-2.247a1.125 1.125 0 0 1 .26-1.431l1.004-.827c.292-.24.437-.613.43-.991a6.932 6.932 0 0 1 0-.255c.007-.38-.138-.751-.43-.992l-1.004-.827a1.125 1.125 0 0 1-.26-1.43l1.297-2.247a1.125 1.125 0 0 1 1.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.086.22-.128.332-.183.582-.495.644-.869l.214-1.28Z"
													/>
													<path
														stroke-linecap="round"
														stroke-linejoin="round"
														d="M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"
													/>
												</svg>
											</button>
										)}
									</div>
								</div>
								<div class="flex-1 overflow-y-auto bg-background text-foreground">
									<div class="flex min-h-full flex-col items-center px-4 py-6">
										{props.children}
									</div>
								</div>
								{props.overlay}
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
