# Security

All transfers are encrypted end-to-end. There is no plaintext mode, even on local networks.

## Encryption Layers

Dukto uses two encryption layers stacked:

1. **QUIC (TLS 1.3)** — the transport layer. Provides the reliable, encrypted UDP channel. Both sides use self-signed certificates, and certificate verification is intentionally skipped (see below).

2. **Noise_XX (ChaCha20-Poly1305 + BLAKE2s + Curve25519)** — the session layer. Provides mutual authentication and forward secrecy. Ephemeral keys are generated per session. This is the layer that actually authenticates the connection.

### Why Two Layers?

QUIC requires TLS, but TLS needs certificates. On a LAN with no CA, self-signed certificates are the only option — which means no meaningful verification. Rather than trying to make TLS work for authentication (pinning self-signed certs, TOFU, etc.), we let TLS handle the transport and use Noise for everything else.

This is not unusual — Signal, WireGuard, and many P2P protocols use Noise for the same reason: it's simpler, more auditable, and designed specifically for this use case.

### The Noise_XX Pattern

Noise_XX is a three-message handshake where both sides exchange ephemeral and static keys:

```
Initiator → Responder: ephemeral key
Responder → Initiator: ephemeral key + static key + proof
Initiator → Responder: static key + proof
```

After the handshake, both sides have a shared symmetric key derived from the Diffie-Hellman exchange. All subsequent messages are encrypted with ChaCha20-Poly1305.

**Why XX and not IK or NK?** XX provides mutual authentication without requiring either side to know the other's public key in advance. Since peers discover each other dynamically via mDNS, there's no opportunity to pre-share keys.

## Trust Model

### Current (LAN MVP)

The trust model is deliberately simple: **every transfer requires explicit user approval.** There is no concept of trusted devices, remembered peers, or auto-accept rules.

When a transfer arrives:
- The receiver sees who's sending (display name), how many items, and the total size
- The receiver must click Accept or Reject
- If they don't respond within 60 seconds, the transfer is automatically rejected
- The dialog cannot be dismissed by clicking outside or pressing Escape — only via the explicit buttons

This is a conscious design choice. File transfer tools that auto-accept (or accept by default with an opt-out) create security risks, especially in shared office networks.

### Future

- **Trusted device list** — remember specific device_ids and auto-accept from them
- **Certificate pinning** — associate a device_id with its Noise static key on first contact (TOFU)
- **Internet mode** — server-assisted pairing with end-to-end encryption preserved

## Path Safety

A malicious sender could try to:
- Write outside the destination directory (path traversal with `..`)
- Overwrite system files (absolute paths like `/etc/passwd`)
- Exploit filesystem quirks (Windows reserved names, null bytes, control characters)

Dukto prevents all of these:
- Relative paths with `..` components are rejected
- Absolute paths (starting with `/` or `\`) are rejected
- Null bytes in paths are rejected
- Invalid characters are replaced with `_`
- Windows reserved names (CON, PRN, NUL, COM1-9, LPT1-9) are prefixed with `_`
- Existing files get conflict suffixes (" (1)", " (2)" etc.) instead of being overwritten

## What Is Not Logged

- File contents are never logged at any level
- Clipboard payloads are never logged
- Noise keys are ephemeral and never persisted
- Transfer metadata (file names, sizes) is logged at debug level only

## What Is Persisted

- **Device identity** (UUID) — persisted to app data directory, survives reinstall if data is preserved
- **Settings** (destination directory, theme) — persisted as JSON in app data directory
- **Nothing else** — no transfer history, no peer list, no keys
