# Desktop Auto-Updates

The desktop app checks a stable update feed at startup and offers a manual check in Settings. It reports whether an update is available and lets the user choose when to download and install it.

## User flow

- The app checks for an update and shows the available version and release notes.
- The user starts the download and can follow byte progress.
- Tauri verifies the downloaded package with the updater public key embedded in the app.
- The user chooses Install and restart after the download finishes.
- If a transfer is active, installation is deferred. The downloaded package remains available to retry when the transfer finishes.
- An incoming transfer request takes priority over the update dialog.

A failed download or invalid signature must not replace the installed version.

## Release artifacts

The release workflow creates platform-specific first-install packages and signed updater payloads. The current workflow definition is the source of truth for its target matrix. CLI archives are separate and are not updater payloads.

The generated feed maps each platform and architecture to its published installer update asset and signature. The feed and assets must come from the same stable release. Drafts and prereleases must not be treated as production updates.

## Maintainer checks

- Keep the signing private key in a protected release environment and out of the repository.
- Validate checksums and verify every updater signature before publishing the feed.
- Test invalid signatures, interrupted downloads, and updates while a transfer is active.
- Confirm that a successful install restarts into the expected version.
- Treat key rotation as a migration: already-installed apps trust the embedded public key, so replacing the signer without a migration can block future updates.
- Do not describe a draft or CI artifact as an available public update.

See [website and release builds](website-and-releases.md) for the contribution workflow and [macOS signing](macos-notarization.md) for Apple package verification.
