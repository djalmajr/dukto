# Build System

## Core Tooling

- **Package manager:** Bun
- **Frontend build:** Vite + `vite-plugin-solid`
- **Desktop shell:** Tauri v2
- **Rust runtime:** Cargo
- **Core networking baseline:** `mdns-sd` + `quinn` + `snow`

## Expected Scripts

Keep the root scripts simple and predictable:

```json
{
  "dev": "tauri dev",
  "build": "tauri build",
  "lint": "biome check . && tsc --noEmit",
  "test": "bun test"
}
```

If the project grows, split commands into smaller scripts, but keep `dev`, `build`, `lint`, and `test` as the stable top-level entrypoints.

## Directory Expectations

- `src/` is the frontend source root
- `src-tauri/` is the Tauri/Rust root
- `dist/` is generated frontend output
- Tauri-generated or build artifacts must stay out of source control unless explicitly required
- Rust modules should be split early into at least `commands`, `discovery`, `protocol`, `transfer`, `crypto`, and `state`

## Verification Order

Before considering a task done, run in this order when applicable:

1. format/lint
2. typecheck
3. Rust checks/tests
4. JS tests
5. production build

If one step is intentionally skipped, state it explicitly in the final report.

## Rust Guidance

- Prefer `tokio` for async runtime if networking is async-heavy
- Keep `main.rs` small; move behavior into modules early
- Separate protocol, discovery, transfer, crypto, and state concerns into distinct modules
- Avoid hiding side effects in constructors; prefer explicit `start_*()` entrypoints
- Validate one real end-to-end LAN transfer as early as possible; do not wait for perfect UI before testing the core stack

## Frontend Guidance

- Keep Vite config minimal until a real need appears
- Add aliases only when the folder structure becomes noisy
- Do not introduce a heavyweight state library unless native Solid primitives become insufficient
- Frontend helpers for Tauri events/invoke should stay thin and typed

## Dependency Policy

- Prefer small, well-scoped dependencies
- Favor platform capabilities already available through Tauri plugins before adding new libraries
- Every networking dependency must have a clear role in the architecture
- Avoid overlapping libraries for the same concern
- Do not add fallback transports that contradict the chosen MVP architecture unless the product decision changes explicitly
