# Development Setup

## Prerequisites

- **Bun** for frontend dependencies, scripts, and tests.
- **Rust** stable toolchain, installed with rustup.
- Platform prerequisites required by Tauri. Linux builds require WebKitGTK and related system libraries; see the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your operating system.

## Install and run

Run these commands from the repository root:

~~~sh
bun install
bun run dev
~~~

The development command runs the Tauri desktop shell with the Vite frontend. Frontend changes hot-reload; Rust changes rebuild the native backend.

The standalone prototype is optional and does not connect to the network or native APIs:

~~~sh
bun run proto:dev
~~~

It uses mock state to preview common host, transfer, and settings flows. Use the desktop app for native dialogs, network transfers, operating-system integration, and final accessibility checks.

## Build and validate

~~~sh
bun run build
bun run lint
bun run test
bun run site:check
bun run site:build
~~~

`bun run build` creates desktop bundles for the current platform. The site commands type-check and build the public website. Platform installers and signed updater artifacts are produced by the release workflow.
