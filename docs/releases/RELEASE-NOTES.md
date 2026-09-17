# Dukto 0.1.1

Dukto is a cross-platform app (desktop and mobile) and command-line tool for transferring files over a local network across macOS, Windows, Linux, iOS, and Android devices.

This preview includes:

- Local device discovery through mDNS.
- Direct file and folder transfers over encrypted QUIC sessions.
- Receiver approval for every incoming transfer.
- A CLI that shares the graphical app's transfer implementation.
- Concurrent transfers, per-transfer progress, and receiver confirmation before the sender reports success.
- Safe per-file staging: incomplete files are removed on cancellation or failure, and completed files are published without replacing existing files.

## Changes in 0.1.1

- Remove a host card about two seconds after mDNS reports that the host left, instead of retaining it for another minute.
- Publish the Dukto website with direct desktop and CLI downloads for every supported architecture.
- Refine responsive navigation and simplify the desktop and CLI download cards.

Update the sender and receiver together. Protocol 0.1 applications are not compatible with the protocol 0.2 completion receipt.

## Current limitations

- Dukto works on a local network; it does not provide internet relay or remote discovery.
- Networks that block multicast may require a direct receiver address in the CLI.
- Interrupted transfers are not resumable. Fully received files in a multi-file transfer remain available if a later item fails.
- Persistent device pairing and trusted-device auto-accept are not available.
- The CLI and desktop app use separate device identities and settings.
