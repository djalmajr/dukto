# UI Prototyping

The standalone prototype runs as a Vite app, uses the same SolidJS components as the desktop application, and supplies mock state in place of Tauri. It can run without compiling Rust or connecting to another device:

~~~sh
bun run proto:dev
~~~

## Why use the prototype?

It speeds up visual iteration and makes it easy to inspect empty, peer, preview, transfer, incoming-request, and error states without starting network services. It can also help check window sizes and light/dark themes.

The prototype imports real app components rather than copies. Keep shared UI components independent from Tauri runtime APIs so they can be used with both production stores and prototype fixtures.

## Prototype shell

The z-proto web component provides desktop-style window chrome and viewport controls. Its toolbar supplies device presets, zoom, and resizing. Application-specific scenario controls belong in the prototype app.

The component uses Light DOM so the app's styles work inside the prototype. Its class names and CSS variables use the zp- and z-proto prefixes to avoid collisions.

## Mock state

The prototype stores its own peers, files, transfers, and theme settings. Scenarios change that state to represent common user flows. The mock store does not perform networking; keep it deterministic so UI changes are quick to review.

Use the desktop app for native file dialogs, actual network transfer, operating-system integration, and final accessibility checks.
