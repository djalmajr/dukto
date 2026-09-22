# Dukto 0.2.3

Dukto is a cross-platform app and command-line tool for direct, encrypted file transfers across macOS, Windows, Linux, Android, and iOS.

## Changes since 0.2.0

- Add experimental, account-free internet transfers with ephemeral invitations, direct peer-to-peer connectivity when available, and encrypted relay fallback.
- Keep an authenticated internet peer available for multiple sequential transfers until either app closes, the connection fails, or the user explicitly disconnects it.
- Reuse the existing encrypted transfer protocol, approval flow, progress reporting, cancellation, directory handling, and partial-file cleanup for internet peers.
- Add a platform-aware title-bar action and a focused invitation dialog for creating or joining an internet connection.
- Use the operated Dukto rendezvous and admission-gated encrypted relay by default, without requiring an account or embedding a static relay credential.
- Add native QR-code image sharing alongside link copying, with visible loading feedback and toast notifications.
- Keep invitation, connection, and transfer errors in transient toasts instead of reserving modal space.
- Remove expired invitations automatically and keep authenticated peers available after the invitation lifetime ends.
- Open the configured destination folder reliably from packaged desktop applications.
- Improve Settings organization, update feedback, responsive dialog sizing, and language controls.
- Improve Android file selection, destination handling, launcher artwork, and Google Play preparation.
- Remove stale LAN peers promptly while preserving peers that reappear during short network-interface changes.
- Harden invitation parsing, session capacity, secret redaction, endpoint verification, and path-safe error reporting.

## Downloads

This GitHub release provides updater-signed desktop installers and CLI archives for the supported macOS, Windows, and Linux architectures. macOS desktop applications are additionally Developer ID signed and notarized when the release workflow completes with notarization enabled. Mobile packages are distributed separately through their platform release channels when available.

Update the sender and receiver together. Internet transfer support is experimental and peers should use the same Dukto version.

## Current limitations

- The operated relay and rendezvous services are enabled by default. They do not receive file contents, but their operator can observe network metadata such as source IP addresses, timing, duration, and traffic volume.
- Direct connectivity depends on both networks. Dukto falls back to the configured encrypted relay when a direct path cannot be established.
- Real Direct routes have not yet been demonstrated on every supported platform and network topology; the encrypted relay fallback is expected on restrictive NAT, CGNAT, firewall, or UDP-blocked networks.
- Internet invitations are temporary and intentionally do not create an account or persist trusted devices.
- Interrupted transfers are not resumable. Fully received files in a multi-file transfer remain available if a later item fails.
- Windows installers are not Authenticode-signed. The updater payload remains cryptographically signed by Dukto's updater key.
- The CLI and graphical app use separate device identities and settings.
