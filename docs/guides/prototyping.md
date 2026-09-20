# UI Prototyping

Prototype interface changes in the main application so that visual work stays aligned with the real Tauri commands, stores, and transfer lifecycle.

~~~sh
bun run dev
~~~

## Focused iteration

Keep presentation logic in components and move deterministic calculations or validation into helpers that can be covered by focused tests. Route-scoped components and stores can model loading, empty, transfer, incoming-request, cancellation, and error states without duplicating production UI.

Use browser-level component tests for deterministic state transitions, then verify native file dialogs, operating-system integration, and network behavior in the desktop or mobile shell. Check light and dark themes, narrow and wide layouts, keyboard navigation, focus movement, and screen-reader labels before considering a flow complete.

## Test fixtures

Fixtures should contain synthetic peers, files, and transfer metadata. Keep them isolated from production state, deterministic, and free of user data. A fixture may replace an external side effect in a focused test, but final validation must exercise the real Rust transport and Tauri boundary.
