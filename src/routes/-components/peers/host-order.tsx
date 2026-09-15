import { For, type JSX, Show, createMemo, createSignal, onCleanup, onMount } from "solid-js";
import { Portal } from "solid-js/web";
import { t } from "~/helpers/i18n";

interface HostOrderProps {
	ids: string[];
	disabled?: boolean;
	label: (id: string) => string;
	onMove: (source: string, target: string, after: boolean) => boolean | Promise<boolean>;
	children: (id: string, header: JSX.HTMLAttributes<HTMLDivElement>) => JSX.Element;
}

export default function HostOrder(props: HostOrderProps) {
	let list!: HTMLDivElement;
	let frame = 0;
	let pointer:
		| {
				id: string;
				x: number;
				y: number;
				startY: number;
				lastY: number;
				element: HTMLDivElement;
				header: HTMLDivElement;
				pointerId: number;
				preview: HTMLElement;
				width: number;
				offsetX: number;
				offsetY: number;
		  }
		| undefined;
	const [draft, setDraft] = createSignal<string[]>();
	const [saving, setSaving] = createSignal(false);
	const [active, setActive] = createSignal<string>();
	const order = createMemo(() => {
		const current = draft();
		return current
			? [
					...current.filter((id) => props.ids.includes(id)),
					...props.ids.filter((id) => !current.includes(id)),
				]
			: props.ids;
	});
	const [preview, setPreview] = createSignal<{
		id: string;
		node: HTMLElement;
		x: number;
		y: number;
		width: number;
	}>();
	const [announcement, setAnnouncement] = createSignal("");

	function cancel(keepOrder = false) {
		cancelAnimationFrame(frame);
		const header = pointer?.header;
		if (pointer?.element.hasPointerCapture(pointer.pointerId))
			pointer.element.releasePointerCapture(pointer.pointerId);
		pointer = undefined;
		if (!keepOrder) setDraft(undefined);
		setPreview(undefined);
		if (header?.isConnected) header.focus({ preventScroll: true });
	}

	function updateTarget() {
		if (!pointer) return;
		const bounds = list.getBoundingClientRect();
		if (Math.abs(pointer.y - pointer.startY) >= 4 || preview()) {
			setPreview({
				id: pointer.id,
				node: pointer.preview,
				x: pointer.x - pointer.offsetX,
				y: pointer.y - pointer.offsetY,
				width: pointer.width,
			});
			if (!props.ids.includes(pointer.id)) {
				cancel();
				return;
			}
			const current = order();
			const source = current.indexOf(pointer.id);
			const down = pointer.y > pointer.lastY;
			const up = pointer.y < pointer.lastY;
			if ((down || up) && pointer.x >= bounds.left && pointer.x <= bounds.right) {
				const rows = [...list.querySelectorAll<HTMLElement>("[data-order-id]")];
				let destination = source;
				for (
					let index = source + (down ? 1 : -1);
					index >= 0 && index < rows.length;
					index += down ? 1 : -1
				) {
					const rect = rows[index].getBoundingClientRect();
					if (
						down ? pointer.y <= rect.top + rect.height / 2 : pointer.y >= rect.top + rect.height / 2
					)
						break;
					destination = index;
				}
				if (destination !== source) {
					const next = current.filter((id) => id !== pointer?.id);
					next.splice(destination, 0, pointer.id);
					setDraft(next);
				}
			}
			pointer.lastY = pointer.y;
			let scroll: HTMLElement | null = list.parentElement;
			while (scroll && !/(auto|scroll)/.test(getComputedStyle(scroll).overflowY))
				scroll = scroll.parentElement;
			if (scroll) {
				const rect = scroll.getBoundingClientRect();
				const delta = pointer.y < rect.top + 32 ? -8 : pointer.y > rect.bottom - 32 ? 8 : 0;
				scroll.scrollTop += delta;
			}
		}
	}

	function track() {
		updateTarget();
		if (pointer) frame = requestAnimationFrame(track);
	}

	async function move(source: string, target: string, after: boolean) {
		const focused = document.activeElement;
		const moved = await props.onMove(source, target, after);
		if (focused instanceof HTMLElement && focused.isConnected && list.contains(focused))
			focused.focus();
		if (moved) setAnnouncement(t("hostMoved", { host: props.label(source) }));
	}

	function headerProps(
		id: string,
	): JSX.HTMLAttributes<HTMLDivElement> & { "data-active"?: string } {
		const enabled = () => !props.disabled && !saving() && props.ids.length > 1;
		return {
			get tabIndex() {
				return !props.disabled && props.ids.length > 1 ? 0 : undefined;
			},
			role: "group",
			get "data-active"() {
				return active() === id ? "true" : undefined;
			},
			onFocus: (event) => {
				if (event.target === event.currentTarget) setActive(id);
			},
			onBlur: () => setActive((current) => (current === id ? undefined : current)),
			get "aria-label"() {
				return t("reorderHost", { host: props.label(id) });
			},
			"aria-keyshortcuts": "ArrowUp ArrowDown",
			get title() {
				return enabled() ? t("reorderHostHint") : undefined;
			},
			onPointerDown: (event) => {
				if (!enabled() || event.button !== 0 || pointer) return;
				if (event.target.closest("button, a, input, [role=button]")) return;
				const card = event.currentTarget.parentElement;
				if (!card) return;
				const rect = card.getBoundingClientRect();
				const snapshot = card.cloneNode(true) as HTMLElement;
				// This is a visual snapshot, never another interactive host or drop target.
				for (const element of [snapshot, ...snapshot.querySelectorAll("*")]) {
					element.removeAttribute("id");
					element.removeAttribute("data-peer-id");
					element.removeAttribute("data-order-id");
				}
				event.preventDefault();
				setActive(id);
				event.currentTarget.focus({ preventScroll: true });
				list.setPointerCapture(event.pointerId);
				setDraft([...props.ids]);
				pointer = {
					id,
					x: event.clientX,
					y: event.clientY,
					startY: event.clientY,
					lastY: event.clientY,
					element: list,
					header: event.currentTarget,
					pointerId: event.pointerId,
					preview: snapshot,
					width: rect.width,
					offsetX: event.clientX - rect.left,
					offsetY: event.clientY - rect.top,
				};
				frame = requestAnimationFrame(track);
			},
			onKeyDown: (event) => {
				if (event.target !== event.currentTarget) return;
				if (event.key === "Escape") {
					cancel();
					return;
				}
				if (!enabled() || (event.key !== "ArrowUp" && event.key !== "ArrowDown")) return;
				event.preventDefault();
				const down = event.key === "ArrowDown";
				const target = props.ids[props.ids.indexOf(id) + (down ? 1 : -1)];
				if (target) void move(id, target, down);
			},
		};
	}

	onMount(() => {
		const cancelOnEscape = (event: KeyboardEvent) => {
			if (event.key === "Escape" && pointer) {
				event.preventDefault();
				cancel();
			}
		};
		document.addEventListener("keydown", cancelOnEscape);
		onCleanup(() => document.removeEventListener("keydown", cancelOnEscape));
	});
	onCleanup(() => cancel());
	return (
		<div
			ref={list}
			class="space-y-2"
			onPointerMove={(event) => {
				if (pointer?.pointerId === event.pointerId) {
					pointer.x = event.clientX;
					pointer.y = event.clientY;
				}
			}}
			onPointerUp={async (event) => {
				if (!pointer || pointer.pointerId !== event.pointerId) return;
				pointer.x = event.clientX;
				pointer.y = event.clientY;
				updateTarget();
				if (!pointer) return;
				const source = pointer.id;
				const next = order();
				const index = next.indexOf(source);
				const target = next[index > 0 ? index - 1 : 1];
				const changed = next.some((id, i) => id !== props.ids[i]);
				const bounds = list.getBoundingClientRect();
				const inside = event.clientX >= bounds.left && event.clientX <= bounds.right;
				cancel(changed && inside);
				if (changed && inside && target) {
					setSaving(true);
					try {
						await move(source, target, index > 0);
					} finally {
						setDraft(undefined);
						setSaving(false);
					}
				}
			}}
			onPointerCancel={(event) => {
				if (pointer?.pointerId === event.pointerId) cancel();
			}}
			onLostPointerCapture={(event) => {
				if (pointer?.pointerId === event.pointerId) cancel();
			}}
		>
			<div class="sr-only" aria-live="polite">
				{announcement()}
			</div>
			<For each={order()}>
				{(id) => (
					<div
						data-order-id={id}
						class="relative"
						classList={{ "opacity-40": preview()?.id === id }}
					>
						{props.children(id, headerProps(id))}
					</div>
				)}
			</For>
			<Show when={preview()}>
				{(snapshot) => (
					<Portal>
						<div
							data-host-drag-preview
							aria-hidden="true"
							inert
							class="pointer-events-none fixed z-50 overflow-hidden rounded-xl opacity-90 shadow-lg"
							style={{
								left: `${snapshot().x}px`,
								top: `${snapshot().y}px`,
								width: `${snapshot().width}px`,
								"max-height": "calc(100dvh - 2rem)",
							}}
						>
							{snapshot().node}
						</div>
					</Portal>
				)}
			</Show>
		</div>
	);
}
