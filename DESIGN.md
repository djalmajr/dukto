# Dukto desktop UI

The approved visual reference is [Desktop Apps, node 130:3784](https://www.figma.com/design/PErcL7vMjgmY3Ou7oXvxob/Desktop-Apps?node-id=130-3784). The user requested alignment with this frame on 2026-09-14, after native transfer testing.

- Window: 600 logical pixels initially, horizontally resizable from 480 to 640; vertical resize down to 360 pixels, with vertical scrolling. Preserve native window controls and disable maximize.
- Header: 45 pixels high, muted surface and bottom border. Center the actual local device name and hostname; settings at the left. Reserve space for native macOS controls.
- Content: full available width, 16-pixel horizontal and 24-pixel vertical padding. Peer cards separated by 8 pixels.
- Cards: 12-pixel radius, 1-pixel neutral border, subtle shadow. Peer header uses 14-pixel padding, a 40-pixel platform tile, 22-pixel glyph, and 12-pixel gap.
- Typography: bundled Inter (SIL OFL; license next to font). Peer name 14/20 semibold; hostname and file details 12/16; muted secondary text. Use the shared semantic theme tokens in both light and dark modes.
- Expanded preview: 12-pixel padding, 208-pixel maximum file-list height with independent vertical scroll, 16-pixel file/folder icons, 36-pixel Send/Cancel buttons separated by 8 pixels. File removal becomes visible on hover or keyboard focus.
- Long content must not widen the window or hide actions permanently. Preserve visible keyboard focus and native file selection/drag-and-drop.

The prototype's mock peers and files are examples. Production displays discovered peers, actual metadata, and an explicit Send confirmation. Drops target the specific peer card under the pointer; drops outside peer cards are ignored.

Transfer actions belong to the available host, using the vertical ellipsis at the right of each card header ([node 130:3801](https://www.figma.com/design/PErcL7vMjgmY3Ou7oXvxob/Desktop-Apps?node-id=130-3801)). The menu contains Add files and Add folders. There is no global selection or global add toolbar. Each host has an independent incremental draft, deduplicated by full path. Send, Cancel and removal affect only that card. Canceling a native picker preserves its draft; results remain attached to the host that opened the picker. When a host leaves discovery, its draft and pending picker results are discarded, never reassigned to another host.

The settings gear sits at the left of the shared title bar. Windows uses custom minimize/disabled-maximize/close controls on the right, with native decorations disabled; macOS keeps native traffic lights and reserves their space before the gear. The device identity remains centered.
