# Security

Dukto encrypts transfers. There is no plaintext transfer mode. Encryption protects the contents in transit, but it does not by itself verify that a peer represents a particular person or device.

## Encryption

Dukto uses QUIC with TLS 1.3 and establishes a Noise session for application data. The QUIC certificates are self-signed and are not verified against a certificate authority. Noise derives session keys, but Dukto does not verify or persist a trusted peer key. An active intermediary may therefore establish separate encrypted sessions with each side.

This design provides encrypted transport, not authenticated identity. Device names, host names, and advertised IDs are labels supplied by the peer. Do not treat them as proof of who is sending a transfer.

The experimental internet path adds invitation possession, authenticated iroh endpoint IDs, transcript-bound pairing, and a bound Noise session. Its rendezvous document is encrypted on the client and relays do not receive plaintext transfer data. Relay operators can still observe IP addresses, timing, duration, and traffic volume. See [Internet Transfers](internet-transfers.md) for the configuration and current release boundary.

## Approval and trust

The graphical app asks you to accept or reject each incoming transfer. The CLI requires the explicit `--accept` option before it automatically accepts transfers. There is no persistent pairing or trusted-device list.

Review the sender and the item list before accepting. An incoming request cannot be accepted by clicking outside it or pressing Escape. Settings dialogs can be closed by clicking their overlay.

## Safe file handling

Received paths are checked to prevent writing outside the selected destination. Dukto rejects traversal, absolute paths, and symbolic links in incoming parent-directory paths, sanitizes names that are invalid on supported systems, and chooses a conflict name instead of overwriting an existing file. The destination selected by the user may itself be a symbolic link.

Each incoming file is staged next to its destination and published only after the complete file has arrived. If a transfer is canceled, disconnected, or fails before that point, Dukto removes the incomplete staged file. Files completed before a batch is interrupted and files that existed before the transfer remain intact. This is per-file safety; a batch is not an all-or-nothing filesystem transaction.

## Data on the device

Dukto stores its device ID and app settings locally. It does not keep a transfer history or a list of trusted peers. File contents are not included in diagnostic logs.
