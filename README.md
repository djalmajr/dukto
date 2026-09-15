# Dukto

Dukto transfers files and folders between macOS, Windows, and Linux computers on the same local network. Devices discover one another automatically; transfers move directly between peers without an account or cloud storage service.

The desktop app and the command-line interface use the same transfer protocol.

## Features

- Local discovery through mDNS.
- Encrypted QUIC transfer sessions using Noise.
- Explicit receiver approval for incoming transfers.
- Files, folders, mixed selections, and concurrent transfers.
- Progress, transfer rates, receipts, and transfer-specific cancellation.
- A CLI for terminal workflows and automation.

Dukto does not currently provide internet relay, persistent trusted-device pairing, or transfer resumption. Display names and hostnames are self-reported; approve requests only from devices you expect.

## Documentation

All documentation body content is maintained in US English. The website may localize its navigation and other interface text.

- [Product scope](docs/PRD.md)
- [Architecture overview](docs/concepts/overview.md)
- [LAN discovery](docs/concepts/discovery.md)
- [Transfer protocol](docs/concepts/transfer-protocol.md)
- [Security model](docs/concepts/security.md)
- [CLI guide](docs/guides/cli.md)
- [Development setup](docs/guides/development.md)
- [Project structure](docs/guides/project-structure.md)
- [Prototyping](docs/guides/prototyping.md)
- [Manual UI testing](docs/guides/manual-ui-tests.md)
- [Concurrent transfer testing](docs/guides/concurrency-validation.md)
- [Website and release builds](docs/guides/website-and-releases.md)
- [macOS signing and notarization](docs/guides/macos-notarization.md)
- [Release notes](docs/releases/RELEASE-NOTES.md)

## Quick start for contributors

Install Bun, Rust, and the Tauri platform prerequisites, then run these commands from the repository root:

~~~sh
bun install
bun run dev
~~~

For the CLI and website, see their guides linked above.

## License

No license has been published. Do not assume permission to redistribute or reuse the code.
