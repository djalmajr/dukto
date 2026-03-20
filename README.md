# Dukto

Cross-platform desktop file transfer for local networks. Zero configuration, end-to-end encrypted, peer-to-peer.

## Table of Contents

- [Concepts](#concepts)
  - [Overview](docs/concepts/overview.md)
  - [Discovery](docs/concepts/discovery.md)
  - [Transfer Protocol](docs/concepts/transfer-protocol.md)
  - [Security](docs/concepts/security.md)
- [Guides](#guides)
  - [Development Setup](docs/guides/development.md)
  - [Prototyping](docs/guides/prototyping.md)
  - [Project Structure](docs/guides/project-structure.md)

## Concepts

### Overview

Dukto discovers nearby devices via mDNS and transfers files directly over QUIC with Noise encryption. No server, no cloud, no accounts. See [Overview](docs/concepts/overview.md).

### Discovery

Devices advertise themselves on the local network using multicast DNS. Peers appear and disappear in real-time. See [Discovery](docs/concepts/discovery.md).

### Transfer Protocol

Framed, versioned packets over QUIC with explicit acceptance, progress tracking, and structured error handling. See [Transfer Protocol](docs/concepts/transfer-protocol.md).

### Security

End-to-end encryption via Noise handshake over QUIC. Every transfer requires explicit user approval. See [Security](docs/concepts/security.md).

## Guides

### Development Setup

Local development with Tauri, SolidJS, and Rust. See [Development Setup](docs/guides/development.md).

### Prototyping

Standalone UI prototyping with the `<z-proto>` web component. See [Prototyping](docs/guides/prototyping.md).

### Project Structure

Module classification, colocation conventions, and architecture decisions. See [Project Structure](docs/guides/project-structure.md).

## Quick Start

```bash
# Install dependencies
bun install

# Development mode (Tauri + frontend)
bun run dev

# Run UI prototype (no Rust needed)
bun run proto:dev
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Desktop framework | Tauri v2 |
| Frontend | SolidJS + TypeScript + Tailwind CSS v4 |
| Backend | Rust (Tokio, Quinn, Snow, mdns-sd) |
| Build | Bun + Vite + Biome |
| Routing | TanStack Router |

## License

Private.
