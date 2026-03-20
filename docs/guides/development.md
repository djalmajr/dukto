# Development Setup

## Prerequisites

- **Bun** — package manager and test runner
- **Rust** — stable toolchain via rustup
- **Tauri CLI** — `cargo install tauri-cli@^2`

On macOS, no additional dependencies. On Linux, you may need webkit2gtk and related system libraries (see Tauri's prerequisites docs).

## Running

```bash
bun install              # install frontend dependencies
bun run dev              # full app: Tauri + SolidJS + Rust (hot reload for both)
bun run proto:dev        # prototype only: standalone Vite server, no Rust needed
```

The prototype is useful for UI iteration — it uses the same real components with mock data, so you can design and test UI flows without compiling Rust or having a second device on the network.

## Building

```bash
bun run build            # production build (Tauri bundles the app)
bun run vite:build       # frontend only (outputs to dist/)
```

## Linting and Testing

```bash
bun run lint             # biome check + TypeScript noEmit
bun run lint:fix         # biome auto-fix (import sorting, formatting)
bun run test             # bun test runner
```

Biome handles import organization, formatting (tabs, double quotes, semicolons), and recommended lint rules. It runs on both the main app and the prototype.

## How Development Mode Works

`bun run dev` starts two processes:

1. **Vite dev server** on port 1420 — serves the SolidJS frontend with hot module replacement
2. **Tauri dev process** — compiles and runs the Rust backend, opens the native window pointing at localhost:1420

Changes to frontend code hot-reload instantly. Changes to Rust code trigger a recompile (typically 2-5 seconds for incremental builds).

## How the Prototype Works

`bun run proto:dev` starts a standalone Vite server (port 3333) that serves the same SolidJS components but wired to mock stores instead of Tauri. The `<z-proto>` web component provides the desktop window chrome with viewport controls, resize handles, and device presets.

The prototype imports real components from the main app via the `~/` alias. This means any component changes in the main app are immediately reflected in the prototype (and vice versa — components are developed and tested in the prototype first, then used in the real app without changes).

## Environment

| Variable | Default | Description |
|----------|---------|-------------|
| `TAURI_DEV_HOST` | — | Override Tauri's dev server host |
| `RUST_LOG` | `dukto_lib=debug` | Rust logging filter (tracing-subscriber) |
