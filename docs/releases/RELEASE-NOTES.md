# Dukto preview release notes

Dukto is a desktop app and command-line tool for transferring files over a local network between macOS, Windows, and Linux computers.

This preview includes:

- Local device discovery through mDNS.
- Direct file and folder transfers over encrypted QUIC sessions.
- Receiver approval for every incoming desktop transfer.
- A CLI that shares the desktop app's transfer implementation.
- Concurrent transfers, per-transfer progress, and receiver confirmation before the sender reports success.
- Safe per-file staging: incomplete files are removed on cancellation or failure, and completed files are published without replacing existing files.

Update the sender and receiver together. Protocol 0.1 applications are not compatible with the protocol 0.2 completion receipt.

## Current limitations

- Dukto works on a local network; it does not provide internet relay or remote discovery.
- Networks that block multicast may require a direct receiver address in the CLI.
- Interrupted transfers are not resumable. Fully received files in a multi-file transfer remain available if a later item fails.
- Persistent device pairing and trusted-device auto-accept are not available.
- The CLI and desktop app use separate device identities and settings.
