# Project Structure

## Organizing Code

The codebase follows two organizing principles:

1. **Generic code lives at the top level** — UI primitives, utilities, and global stores are in root-level directories. They're used everywhere and don't belong to any specific feature.

2. **Domain-specific code is colocated with routes** — components and stores that serve a specific feature live inside the routes directory, using TanStack Router's `-prefix` convention (directories prefixed with `-` are ignored by the router).

This means looking at the routes directory tells you both what pages exist and what domain logic powers them. When the app grows and a domain gets its own routes, its components naturally move with it.

## Module Classification

Code outside of components and stores is classified into three categories:

- **Helpers** — app-specific functions that wrap external dependencies or configure libraries. Examples: i18n setup, Tauri command wrappers. You couldn't drop these into another project without modification.

- **Utils** — generic, stateless functions that could be used in any project. Examples: class name merging, byte formatting, platform detection. No app-specific logic.

- **Libs** — reusable modules that could be extracted as separate packages. Currently empty, but would contain things like a crypto wrapper or a custom SDK client.

The distinction matters because it tells you what's safe to change independently. Renaming a util function is safe everywhere. Changing a helper might affect app behavior. Adding a lib is adding a dependency.

## Colocation Conventions

TanStack Router uses directory name prefixes to control routing behavior:

- **`-components/`**, **`-stores/`**, **`-helpers/`** — prefixed with `-`, invisible to the router. Use for code colocated with routes.
- **`_domain/`** — prefixed with `_`, creates a pathless layout route. Use for grouping routes by domain when sub-routes appear.

Currently the app has a single route (`/`), so all domain code lives in `routes/-components/` and `routes/-stores/`. When a feature needs its own routes (e.g., a settings page at `/settings`), its components move from `routes/-components/settings/` to `routes/_settings/-components/`.

## State Management Approach

The app uses SolidJS primitives directly:

- **`createResource`** for data that loads once (device identity, initial settings)
- **`createStore`** for structured reactive state (peers map, transfers map)
- **`createSignal`** for simple values (theme, language, UI toggles)

No external state management library. The stores are thin wrappers around Tauri event listeners — they convert events into reactive state that components can subscribe to.

Global stores hold cross-cutting state (device identity, discovered peers, user settings). Domain stores hold feature-specific state (active transfers, incoming requests). The boundary is practical: if multiple unrelated features need it, it's global. If only the transfer UI needs it, it's domain.

## Component Purity

Shared components (everything in the UI primitives and the colocated `-components/` directories) must be **pure** — they receive all data via props and emit actions via callbacks. They never import stores or runtime APIs directly.

This rule exists for one reason: the prototype. Components that import stores can't be used in the prototype (which has its own mock stores). Components that take props can be used anywhere.

The boundary where props become store reads is in the route files — the "pages" that wire stores to components. This is the only place where store imports and component rendering meet.
