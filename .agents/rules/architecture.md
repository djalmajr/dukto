# Architecture

## Overview

Desktop-first Tauri v2 app for cross-platform file transfer. **Tech stack:** Tauri v2, Rust core, SolidJS, solid-ui, Tailwind CSS v4, Bun tooling.

The product has two network modes:

1. **LAN mode** - zero-config local discovery via `mDNS`, secure transfer via `QUIC + Noise`
2. **Internet mode** - pairing via signaling, direct P2P when possible, relay fallback when needed

## Product Direction

- **Near term:** desktop MVP for LAN transfers with `mDNS + QUIC + Noise`
- **Next:** internet pairing with signaling + relay fallback
- **Later:** mobile, always-on sharing, favorites, trusted devices

If implementation choices conflict with this order, preserve the chosen LAN architecture instead of downgrading to ad-hoc shortcuts.

## High-Level Structure

```text
dukto/
├── src/                    # SolidJS frontend
├── src-tauri/              # Tauri shell + Rust runtime
│   └── src/
│       ├── commands/       # Tauri command handlers
│       ├── protocol/       # Message formats and transfer framing
│       ├── discovery/      # mDNS peer discovery and presence
│       ├── transfer/       # Send/receive pipeline
│       ├── crypto/         # Noise handshake and trust helpers
│       ├── state/          # Shared app/runtime state
│       └── platform/       # OS integration helpers
├── docs/                   # Product and technical docs
└── .agents/rules/          # Project-specific working rules
```

## Boundary Rules

- **Frontend owns presentation**: layout, interaction state, drag/drop state, list rendering, progress views, accept/reject flows.
- **Rust owns system and network concerns**: discovery, transport, cryptography, session orchestration, filesystem access, notifications, persistence.
- **Tauri commands/events are the bridge**: commands for request/response, events for peer updates, trust state, progress, completion, and errors.
- **Do not move transfer logic into the web layer.** The frontend must never become the source of truth for networking or filesystem writes.

## Runtime Model

- **Discovery loop** runs in Rust and pushes peer updates to the UI.
- **Transfer sessions** are stateful objects with ids, lifecycle, trust status, progress, errors, and cancellation.
- **Shared app state** is centralized in Rust and mirrored into Solid stores via events.
- **Long-running work** must be async and non-blocking.

## Networking Direction

### LAN

- Use **mDNS** as the official LAN discovery mechanism.
- Use **QUIC** as the official LAN transport.
- Use **Noise** handshake before transfer payload exchange.
- Do not add alternative LAN discovery/transports unless they are explicit product decisions.

### Internet

- Use a **separate signaling layer** from the transfer layer.
- Design the app so future internet transport can switch between **WebRTC data channel** and **relay fallback** without changing UI contracts.
- Keep relay concerns isolated so internet mode can evolve without rewriting the LAN flow.

## Protocol Principles

- Discovery protocol should be **small, explicit, and versionable**.
- Transfer protocol should support **metadata packets + binary chunks**.
- Include stable identifiers for **device id**, **transfer id**, and **item id**.
- Trust and fingerprint information should be first-class session concepts, not UI-only decorations.

## Persistence

- Persistence is optional for the earliest LAN MVP beyond user settings.
- Persist only user-relevant state: settings, trusted devices later, default destination folder, optional history.
- Do not persist transient transfer buffers or peer presence snapshots beyond what the UX needs.

## References

- `docs/PRD.md` - product direction and phased roadmap
- `.agents/plans/1773846250998-proud-garden.md` - current implementation plan
- `~/Developer/github/dukto-qt5` - LAN UX and direct-transfer reference
- `~/Developer/github/nitroshare-desktop` - discovery and protocol modeling reference
