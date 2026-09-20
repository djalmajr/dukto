# Internet Transfers (Experimental)

Dukto's source tree includes an experimental account-free internet transfer path. It complements the existing LAN flow; it does not replace mDNS discovery or the LAN listener. A production Dukto relay and rendezvous service has not been deployed yet, so released builds must not be assumed to provide internet reachability.

## User flow

The receiving device creates a short-lived invitation and can share it as a QR code or through an explicit copy-link action. The sender scans or pastes that deep link, or imports the invitation and confirms the displayed eight-digit code for manual pairing. After the devices authenticate the invitation and each other's transport endpoint, the sender selects files or folders. The receiver still accepts or rejects the transfer before file bytes flow.

An invitation is ephemeral and is consumed by one authenticated session. Dukto does not create an account, trusted-device list, transfer history, or asynchronous download link.

## Connectivity

Dukto tries an iroh direct path first. When direct connectivity is unavailable, configured iroh relays can carry the same encrypted stream. The transfer protocol, approval, progress, receipts, conflict handling, and partial-file cleanup are shared with LAN transfers.

Development and operator builds can configure the transport before the app starts:

- `DUKTO_RELAY_URLS` is a comma-separated list of one to eight HTTPS relay URLs. When it is absent, the internet endpoint is direct-only. An empty or invalid configured value is an error.
- `DUKTO_RENDEZVOUS_URL` is the HTTPS base URL of the opaque invitation rendezvous API. When it is absent, the invitation's embedded routes are used. An empty or invalid configured value is an error.
- `DUKTO_FORCE_RELAY_ONLY=1` disables direct transports for a controlled relay validation. It is rejected unless `DUKTO_RELAY_URLS` is also configured. Normal builds should leave it unset so direct connectivity remains preferred.

The rendezvous client rejects redirects, applies bounded connection and request timeouts, and limits response bodies. The relay and rendezvous configuration must be compatible on both devices. These environment variables are operator settings, not an end-user self-hosting requirement.

## Security and privacy

The invitation contains a random secret. A typed code is used through SPAKE2 rather than as a bearer token or direct encryption key. Pairing binds the session ID, roles, expiration, fresh nonces, protocol version, ALPN, and both transport endpoint IDs. The first transfer stream then establishes a transcript-bound Noise session before any transfer header is accepted.

The rendezvous service receives an opaque slot and an encrypted address envelope. It does not need file names, relative paths, approval decisions, or file contents. A relay forwards encrypted traffic, but it can still observe network metadata such as source IP, timing, connection duration, and byte volume. End-to-end encryption does not hide that metadata.

Review the device and transfer summary before accepting. The invitation authenticates possession and the connected transport endpoint; it does not establish a person's legal or real-world identity.

## Current validation boundary

Automated tests cover direct and forced-relay transport, authenticated pairing, file and folder integrity, approval, rejection, cancellation, conflicts, and cleanup. Native validation on two independent networks and production service deployment remain release gates. Do not treat a local build or a configured development relay as evidence that the public service is available.
