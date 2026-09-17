# Project Structure

The repository separates the cross-platform application frontend, its Rust backend, the standalone prototype, and the public website.

## Main areas

- `src/` contains the SolidJS frontend (for desktop and mobile). Reusable interface primitives are in `src/components/`; app-level helpers, stores, and utilities are in their respective directories.
- `src/routes/` contains the main screen and route-scoped features. TanStack Router ignores directories and files prefixed with `-`, so `-components/`, `-stores/`, and `-helpers/` can colocate route implementation without becoming routes.
- `src-tauri/` contains the Rust backend, commands exposed to the frontend, and the CLI binary. Transfer protocol and filesystem behavior live in the backend.
- `src-proto/` is a standalone Vite prototype for visualizing application flows with sample state.
- `site/` contains the public Solid/Vite website, including the English documentation articles.

## State and side effects

Stores under `src/stores/` hold state shared across features, such as discovered peers, settings, and updater status. Route-scoped stores live under `src/routes/-stores/`. Helpers wrap app-specific operations; utilities are stateless functions that can be reused without Dukto-specific state.

Frontend commands cross the Tauri boundary through helpers. Keep filesystem access, networking, protocol handling, and other native side effects in the Rust backend. Keep UI state transitions explicit and covered by focused tests when they affect host ownership or transfer behavior.

## Routes and components

`src/routes/index.tsx` is the main screen. Feature components are grouped under `src/routes/-components/`; shared primitives live under `src/components/`. Add new route files using the existing TanStack Router conventions and allow the router plugin to generate its route tree instead of editing generated output.

Some feature components intentionally connect to app stores or native helpers. Keep responsibilities clear: presentation belongs in components, shared state belongs in stores, and reusable calculations or validation belong in helpers or utilities.
