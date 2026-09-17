# Dukto 0.2.0

Dukto is a cross-platform app and command-line tool for direct, encrypted file transfers across macOS, Windows, Linux, Android, and iOS.

## Changes in 0.2.0

- Add Android and iOS platform support to the shared Tauri application, including bidirectional interoperability coverage with the desktop protocol.
- Keep desktop update controls off mobile builds and add a privacy-policy link to the application settings.
- Adopt `app.dukto` as the application identifier for future desktop and mobile distribution.
- Update the website for all five supported platforms, direct-device terminology, improved documentation navigation, and clearer inline command and download links.
- Add repeatable Android project configuration for multicast discovery permissions and SDK validation.

## Downloads

This GitHub release provides signed desktop installers and CLI archives for the supported macOS, Windows, and Linux architectures. Mobile packages are distributed separately through their platform release channels when available.

Because the application identifier changed to `app.dukto`, an existing 0.1.x desktop installation may require a manual install of 0.2.0 instead of an in-app update.

Update the sender and receiver together. Protocol 0.1 applications are not compatible with the protocol 0.2 completion receipt.

## Current limitations

- Automatic discovery uses the local network. The CLI can connect directly to a known receiver address when discovery is unavailable; internet relay and remote discovery are not available yet.
- Networks that block multicast may require a direct receiver address in the CLI.
- Interrupted transfers are not resumable. Fully received files in a multi-file transfer remain available if a later item fails.
- Persistent device pairing and trusted-device auto-accept are not available.
- The CLI and graphical app use separate device identities and settings.
