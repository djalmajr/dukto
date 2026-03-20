# Frontend

## Stack

SolidJS, TypeScript, Tailwind CSS v4, solid-ui, Bun, Tauri JavaScript API.

Use `~/Developer/zomme/platform/apps/skedly` as a reference for project structure and Solid conventions, but preserve this app's own product language: lightweight, transfer-focused, and utility-first.

## Frontend Responsibilities

- Device list and presence rendering
- Drag and drop interactions
- Transfer queue presentation
- Pairing and connection flows
- Incoming transfer approval and trust UI
- Settings and optional history screens
- Visual feedback for progress, success, and failure

Do not implement socket logic, filesystem writes, cryptographic handshakes, or protocol assembly in the frontend.

## SolidJS Conventions

- `class`, not `className`
- `onInput` for text input changes
- Prefer `<Show>`, `<For>`, `<Switch>` / `<Match>` for control flow
- Do not destructure props directly; use `props.*` or `splitProps()`
- Keep component order: state -> derived values -> handlers/effects -> JSX
- No blank lines inside JSX blocks

## UI Direction

- **Minimal and lightweight** by default
- Desktop-first for now, but keep layouts responsive enough for future Tauri mobile work
- Favor fast scanning: clear hierarchy, visible status, obvious drop targets
- Use purposeful empty states and transfer states; avoid generic placeholder cards everywhere
- Use system-native affordances where that improves clarity: file picker, notifications, tray feedback
- Trust and acceptance states must be obvious: who is sending, what is being sent, and what action the user can take

## Component Library (solid-ui)

- **Always use solid-ui components** (`src/components/ui/`) for buttons, inputs, dialogs, tabs, toggles, and any other UI primitive. Never use raw HTML elements when a solid-ui component exists.
- If a needed component does not exist in `src/components/ui/`, create it following the solid-ui pattern: Kobalte primitive + `cn()` + semantic design tokens. Then use the new component.
- Never inline ad-hoc styled elements that duplicate what a solid-ui component already provides (e.g. a styled `<button>` when `<Button>` exists, a styled `<input>` when `<TextFieldInput>` exists).
- Reference Skedly's `@zomme/ui` for component API conventions when creating new solid-ui components.

## Dialog / Modal Behavior

- Modals must only close via explicit user action: clicking a close button (X), a cancel/reject button, or a confirm/accept button.
- Never dismiss on overlay click or Escape key. Use `onInteractOutside` and `onEscapeKeyDown` with `preventDefault()` on the Kobalte `DialogContent`.
- Keep `onOpenChange` functional so the X (CloseButton) works: `onOpenChange={(open) => !open && onClose()}`.

## Styling Rules

- Centralize colors, radius, spacing, and motion as CSS variables early
- Prefer semantic tokens over repeated raw values
- Tailwind utilities are the default; avoid ad-hoc inline styles
- Use a clear visual direction, but keep the base neutral and utility-driven for a file transfer tool
- Focus states and keyboard accessibility are required for interactive controls

## Module Classification

Use dedicated directories for each module type:

| Directory | Purpose | State | Example |
|-----------|---------|-------|---------|
| `helpers/` | App-specific functions and config wrappers | Stateless | i18n setup, Tauri bridge, form validation |
| `libs/` | Reusable across projects, no business logic | Stateless | Crypto wrapper, SDK client, data structures |
| `utils/` | Generic reusable functions | Stateless | formatDate(), cn(), generateUUID() |

Domain-specific helpers can also be colocated in `routes/-helpers/` when tied to a specific route group.

## Colocation Convention

Use TanStack Router directory conventions:
- `-prefix/` directories are ignored by the router — use for colocated code (`-components/`, `-stores/`, `-helpers/`)
- `_prefix/` directories create pathless layout routes — use for domain grouping when routes appear

**Generic** code lives at `src/` level (`components/`, `helpers/`, `utils/`, `stores/`).
**Domain-specific** code is colocated inside `routes/` (`routes/-components/`, `routes/-stores/`).

When a route group gets its own routes (e.g. `_transfers/send.tsx`), move its components from `routes/-components/` to `routes/_transfers/-components/`.

## Suggested App Structure

```text
src/
├── components/          # generic: ui primitives, shared components
│   └── ui/
├── helpers/             # app-specific wrappers (i18n, Tauri bridge)
├── utils/               # generic stateless functions (cn, format, platform)
├── stores/              # global stores (device, peers, settings)
├── styles/
└── routes/
    ├── -components/     # domain components colocated with routes
    │   ├── peers/
    │   ├── settings/
    │   └── transfers/
    ├── -stores/         # domain stores colocated with routes
    ├── __root.tsx
    ├── index.tsx
    └── locales/
```

## State Management

- Keep UI state close to the feature when possible
- Use shared stores only for cross-screen concerns like peers, active transfers, trust state, and app settings
- Tauri event listeners should feed stores through thin adapters, not directly from components

## Tauri Integration

- Wrap `invoke` and event subscriptions in small frontend helpers
- Keep payload parsing and event naming centralized
- UI components should consume typed helpers/stores, not raw Tauri APIs
- Every incoming transfer flow should expose explicit `accept`, `reject`, and progress states

## Accessibility

- All primary actions must be reachable by keyboard
- Drop zones need visible hover/focus states and text labels
- Progress and error states should have text, not color only
- Minimum touch target remains 44px for future mobile support
- Incoming requests must clearly identify sender, file/folder count, and destination action
