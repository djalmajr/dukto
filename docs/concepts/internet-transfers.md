# Internet Transfers (Experimental)

Dukto's source tree includes an experimental account-free internet transfer path. It complements the existing LAN flow; it does not replace mDNS discovery or the LAN listener. Dukto operates an ephemeral rendezvous and encrypted relay service, but released builds must not be assumed to provide internet reachability until the cross-platform release gates are complete.

## User flow

The receiving device creates a short-lived invitation and can share it as a QR code or through an explicit copy-link action. The sender scans or pastes that deep link, or imports the invitation and confirms the displayed eight-digit code for manual pairing. After the devices authenticate the invitation and each other's transport endpoint, the sender selects files or folders. The receiver still accepts or rejects each transfer before file bytes flow.

An invitation is ephemeral and is consumed by one authenticated session. Once connected, the remote device stays available like a LAN peer and the authenticated connection can carry multiple sequential transfers. Completing, rejecting, or canceling one transfer does not disconnect that peer. The connection ends when either app closes, the transport fails, or a user explicitly selects **Disconnect**. Dukto discards the invitation secret, pairing code, copied link, and advertised invitation routes after authentication; it does not create an account, trusted-device list, transfer history, or asynchronous download link.

## Connectivity

Dukto tries an iroh direct path first. When direct connectivity is unavailable, configured iroh relays can carry the same encrypted stream. The transfer protocol, approval, progress, receipts, conflict handling, and partial-file cleanup are shared with LAN transfers.

Normal builds use `https://relay.djalmajr.dev/` and `https://rendezvous.djalmajr.dev/v1` while still preferring a direct path. Development and operator builds can override that configuration before the app starts:

- `DUKTO_RELAY_URLS` is a comma-separated list of one to eight HTTPS relay URLs. An empty or invalid configured value is an error.
- `DUKTO_RELAY_AUTH_TOKEN` is an optional operator-only bearer token for a private test relay. The operated public flow does not use or embed this token. It is rejected when no relay URL is configured, and it is never written to logs or serialized invitation views.
- `DUKTO_RENDEZVOUS_URL` overrides the HTTPS base URL of the invitation rendezvous API. An empty or invalid configured value is an error.
- `DUKTO_DISABLE_OPERATED_SERVICES=1` disables both operated defaults for offline, direct-only, or custom-service testing. Explicit relay and rendezvous overrides remain available.
- `DUKTO_FORCE_RELAY_ONLY=1` disables direct transports for a controlled relay validation. It is rejected unless `DUKTO_RELAY_URLS` is also configured. Normal builds should leave it unset so direct connectivity remains preferred.

The rendezvous client rejects redirects, applies bounded connection and request timeouts, and limits response bodies. The relay and rendezvous configuration must be compatible on both devices. These environment variables are operator settings, not an end-user self-hosting requirement.

## Security and privacy

The invitation contains a random secret. A typed code is used through SPAKE2 rather than as a bearer token or direct encryption key. Pairing binds the session ID, roles, expiration, fresh nonces, protocol version, ALPN, and both transport endpoint IDs. Every transfer uses a fresh bidirectional stream and a fresh transcript-bound Noise handshake before any transfer header is accepted, while the underlying authenticated iroh connection remains reusable.

The rendezvous service receives an opaque slot, an encrypted address envelope, and the two temporary iroh endpoint IDs needed to admit the invitation owner and joiner. A separate capability derived from the invitation secret proves the joiner's right to consume the slot without revealing the envelope key. The relay asks the gateway whether each endpoint is currently admitted; the private callback uses a machine-to-machine credential that is never shipped in the app. Admissions expire with the invitation and the envelope is consumed once.

The services do not receive file names, relative paths, approval decisions, or file contents. They can still observe network metadata such as source IP, temporary endpoint IDs, timing, connection duration, and byte volume. End-to-end encryption does not hide that metadata. Because Dukto is open source and account-free, the protocol is not binary attestation: abuse is bounded with short lifetimes, one-shot invitations, request limits, and relay capacity controls rather than a secret embedded in the client.

Review the device and transfer summary before accepting. The invitation authenticates possession and the connected transport endpoint; it does not establish a person's legal or real-world identity.

## Current validation boundary

Automated tests cover direct and forced-relay transport, admission denial, authenticated pairing, file and folder integrity, approval, rejection, cancellation, conflicts, and cleanup. The operated service has passed an admission-gated 128 KiB relay smoke on macOS without a static client token. Native Windows validation, full app validation, and the remaining release checks are still release gates. Do not treat the deployed canary as a published application release.
