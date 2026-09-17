# Dukto App UI

This document defines the community-facing interaction and layout contract for the application across desktop and mobile platforms.

## Window and title bar

- Start at 600 logical pixels wide. Allow horizontal resize from 480 to 640 pixels on desktop.
- Allow vertical resizing down to 360 pixels and scroll the content vertically when needed.
- Preserve native window controls and disable maximize on desktop.
- Keep the title bar 45 pixels high with a muted surface and bottom border.
- Center the local device name and hostname.
- Place the settings gear on the right on macOS, iOS, Android, and Linux, and on the left on Windows.
- Preserve space for native macOS traffic lights. Windows uses custom minimize, disabled-maximize, and close controls on the right when native decorations are disabled.

## Content and host cards

- Use 16 pixels of horizontal content padding and 24 pixels of vertical padding.
- Separate host cards by 8 pixels.
- Cards use a 12-pixel radius, a neutral 1-pixel border, and a subtle shadow.
- A host header uses 14-pixel padding, a 40-pixel platform tile, a 22-pixel glyph, and a 12-pixel gap.
- Use bundled Inter with the included SIL Open Font License. Host names are 14/20 semibold; hostnames and file details are 12/16 and muted.
- Long names must not widen the window or permanently hide actions.
- Use shared semantic theme tokens for light and dark modes.

The prototype's peers and files are examples. The product shows discovered peers and actual selected items.

## Host-scoped actions

Every file action belongs to a specific available host. The vertical ellipsis on that host contains Add files and Add folders. Do not add a global file-selection toolbar.

Each host has an independent incremental draft, deduplicated by full path. Send, cancel, and item removal affect only that host. Canceling a native picker preserves the draft. Picker results remain attached to the host that opened it. When a host leaves discovery, discard its draft and pending picker results; never assign them to a different host.

Drops target the host card under the pointer. Ignore drops outside host cards.

## Host ordering

The entire host header is the drag surface, including the icon, device name, hostname, and surrounding blank area. The ellipsis remains an interactive control with its own action.

Show a preview of the host card under the pointer and reorder the list as the preview moves. Commit the new order on drop. Escape or pointer cancellation restores the prior order. Clicking a host header focuses it and shows a subtle background highlight, without a heavy outline. Keyboard Up and Down reorder that host and preserve its focus; keep transfer rows and drafts attached to stable host IDs.

## Transfer preview and progress

- An expanded preview uses 12-pixel padding.
- The file list is at most 208 pixels high and scrolls independently.
- File and folder icons are 16 pixels.
- Send and Cancel buttons are 36 pixels high with an 8-pixel gap.
- File removal appears on hover or keyboard focus.
- Show separate progress, rate, and cancellation controls for each transfer.
- Keep send and receive states distinguishable by color and text.
- Do not remove visible keyboard focus from interactive controls.

## Dialog behavior

Settings has an overlay and closes through its close control or a click on the overlay. Incoming transfer requests require an explicit accept or reject action and do not dismiss through the overlay.

Use native file selection and drag and drop while keeping all actions accessible by keyboard.

## Website header

Use a compact 32-pixel logo and 24-pixel wordmark. On compact layouts, show only the logo and download icon, retaining accessible names. Keep navigation links visible after compacting the controls; use the menu button only at narrow widths where the links and controls no longer fit. Keep light and dark themes consistent.

The website mobile navigation is a compact dropdown aligned with its menu button. Size it to its links with modest padding; do not stretch it across the viewport or repeat the download action inside it.

Keep the website navigation centered independently of the logo and action controls whenever there is room. Icon-only download buttons have no text-spacing margin on the icon.

Header icon-only action buttons use 32-by-32-pixel boxes matching the logo, with centered glyphs.

Download cards follow their content height without stretching empty space. Package rows use labeled icon-only download actions, and installation guides are plain links.

The CLI download section places its introduction above three platform cards in a single row, stacking on narrow screens; avoid a two-column grid that leaves an empty fourth cell.

The header download link indicates the active Downloads page with the navigation accent and a subtle background, including its icon-only state.

Use the Lucide cloud-download glyph consistently for website download actions in the header, calls to action, desktop packages, and CLI archives.

Downloads belongs in the main website navigation alongside Home and Documentation, with the same active-link treatment. It is not a separate header action. This supersedes the earlier icon-only header download treatment.
