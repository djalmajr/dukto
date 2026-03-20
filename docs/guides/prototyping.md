# Prototyping

The prototype is a standalone Vite app that uses the same real components as the main app, wired to mock data instead of Tauri. It runs without Rust, without a second device, and without compiling anything — just `bun run proto:dev`.

## Why a Separate Prototype

UI iteration in a Tauri app is slow: you need Rust compiled, mDNS running, and ideally two devices on the network to see real interactions. The prototype eliminates all of that. You can:

- Test every screen state (empty, peers visible, transfer in progress, error, incoming request) by cycling through scenarios
- Switch between device presets (iPhone, iPad, Desktop, etc.) to check responsive behavior
- Toggle dark/light theme instantly
- Capture any state directly to Figma via HTML-to-Design

Because the prototype imports real components (not copies), any design work done in the prototype is automatically production-ready.

## The `<z-proto>` Web Component

The prototyping shell is a vanilla JS web component that provides a macOS-like window chrome. It's framework-agnostic — works with SolidJS today but could be used with any framework.

**What it provides:**

- macOS-style title bar with traffic lights and customizable title
- Toolbar with 17 device presets, manual width/height inputs, and 5 zoom levels
- 8-direction drag-to-resize with pointer capture
- Light and dark theme support (reacts to `.dark` class on `<html>`)
- Figma capture integration (auto-injects the capture script when `figma-key` is set)
- Self-contained CSS via `adoptedStyleSheets` — no `<link>` tag needed

**What it doesn't provide:**

- Scenario management — that's the app's responsibility (the toolbar area is a slot)
- Content styling — the app's Tailwind classes apply normally inside the window
- Dark mode toggle — the app manages theme state, the web component just reacts

The component uses Light DOM (not Shadow DOM) so the Figma capture script can read the entire DOM tree. Internal classes are prefixed with `zp-` and CSS variables with `--z-proto-` to avoid collisions.

## Scenario Toolbar

Scenarios are defined in the root layout and controlled via a `<select>` with left/right step arrows. Each scenario calls store functions to jump to a specific app state (e.g., "Show peers" calls `showPeers()` + `setScreen({ id: "idle" })`).

The toolbar lives in `<z-proto-header>`, which renders below the main viewport controls. This separation keeps the toolbar controls (presets, zoom) from the scenario controls (app-specific) visually distinct.

## Figma Integration

When the `figma-key` attribute is set on `<z-proto>`, a "Figma" link appears in the toolbar. Clicking it activates the HTML-to-Design capture script (auto-injected by the web component), which reads the DOM and sends it to the specified Figma file as editable layers.

This is useful for:
- Maintaining a design spec that matches the actual implementation
- Generating assets for documentation or presentations
- Comparing design iterations side by side in Figma

## Mock Store

The prototype has its own store with mock data (peers, files, transfers) and a state machine that simulates all user flows. The mock store exports the same function signatures as the real stores, so components can't tell the difference.

The mock store is intentionally simple — no actual networking, no timers for progress animation (you manually tick progress via scenarios). This keeps the prototype fast and deterministic.
