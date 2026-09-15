# Dukto 0.1.0 — preview

Desktop app and CLI for local file transfers across macOS, Windows and Linux.

- Single files, multiple files, folders and mixed selections through the CLI.
- mDNS discovery and encrypted QUIC + Noise sessions, protocol 0.2.
- Receiver confirmation before a sender reports success.

Update both devices together: protocol 0.1 clients are incompatible with the new receipt handshake.

The separate CLI archives and Windows installers are unsigned. The macOS desktop signing status is appended by the installer workflow; existing evaluation builds remain unsigned. Linux compatibility depends on the build baseline (x64 Ubuntu 22.04, arm64 Ubuntu 24.04 in CI; local arm64 evaluation build Ubuntu 26.04). Match the architecture to your machine and check SHA256SUMS before installing.

Known limits: Windows and its own WSL2 did not discover each other in the test topology; direct-address transfers worked. Interrupted transfers are not resumable. Persistent trusted-device pairing is not implemented. The 60-transfer CLI matrix was supplemented by macOS↔Windows UI tests that transferred large and small files concurrently in both directions and verified their hashes. Direct-address CLI → desktop was also validated on macOS (2 GiB) and Windows (1 GiB), including sender identity, progress and matching SHA-256. Desktop → CLI remains unverified.
