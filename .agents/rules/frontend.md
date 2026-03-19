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

## Styling Rules

- Centralize colors, radius, spacing, and motion as CSS variables early
- Prefer semantic tokens over repeated raw values
- Tailwind utilities are the default; avoid ad-hoc inline styles
- Use a clear visual direction, but keep the base neutral and utility-driven for a file transfer tool
- Focus states and keyboard accessibility are required for interactive controls

## Suggested App Structure

```text
src/
├── components/
├── features/
│   ├── peers/
│   ├── transfers/
│   ├── pairing/
│   ├── trust/
│   └── settings/
├── stores/
├── lib/
├── routes/
├── styles/
└── index.tsx
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
