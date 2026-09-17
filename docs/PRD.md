# Product Scope

## Summary

Dukto is a cross-platform file-transfer app for devices on the same local network. It pairs automatic peer discovery with direct, encrypted transfers and an explicit receive decision. A command-line interface supports terminal workflows and automation through the same transfer protocol.

The product prioritizes a small, understandable workflow: open Dukto on both devices, select a destination host, choose files or folders, and approve the incoming request on the receiving device.

## Current scope

The current product is a cross-platform LAN transfer app for macOS, Windows, Linux, iOS, and Android, alongside a desktop CLI for terminal workflows.

- mDNS advertises and discovers available peers on the local network.
- QUIC carries transfer sessions; Noise encrypts the application session.
- The receiver explicitly accepts or rejects each incoming transfer.
- Files, folders, mixed selections, and empty directories are supported.
- The UI and CLI can run multiple transfers concurrently.
- The receiver reports completion to the sender; progress alone is not proof of delivery.
- Destination paths are validated and conflicting filenames are kept distinct.
- The UI and CLI expose transfer progress and errors.

Both computers need compatible protocol versions. Networks that block multicast may prevent automatic discovery. A direct CLI endpoint can help distinguish discovery problems from transfer connectivity.

## Product principles

### Local and direct by default

The LAN flow sends data directly between computers. Dukto does not upload LAN transfers to a cloud storage service or require an account.

### Receiver control

Every incoming transfer in the application requires an explicit decision. CLI unattended receive requires an explicit command-line option for that process. There is no persistent auto-accept rule.

### Honest identity and security

Transfer sessions are encrypted, but the current product does not provide persistent trusted-device pairing. Usernames and hostnames are labels supplied by the sender, not verified identities. See the [security model](concepts/security.md).

### Clear progress and completion

Each transfer has its own state and progress. The sender reports success only after a matching receiver acknowledgment. Errors are associated with the transfer that caused them.

### Focused architecture

The Tauri frontend presents application state and interactions. Rust owns discovery, networking, cryptography, filesystem access, and runtime state. See the [architecture overview](concepts/overview.md).

## Not currently included

Internet pairing or relay, resumable transfers, persistent trusted devices, transfer history, groups, and automatic receive rules are not current product capabilities. No delivery dates are promised for these ideas.

## Community contributions

Start with the [development setup](guides/development.md), then read the [project structure](guides/project-structure.md) and relevant protocol documentation. Use the testing guides to validate changes against real user-visible behavior. Keep changes aligned with the current LAN-first scope unless the product direction is explicitly revised.
