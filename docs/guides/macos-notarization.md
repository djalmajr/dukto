# macOS Signing and Notarization

This guide is for maintainers who build macOS distribution packages. Local development builds do not need Apple signing credentials.

## Requirements

- An Apple Developer account with a Developer ID Application certificate.
- An App Store Connect API key authorized to submit software for notarization.
- A protected CI environment configured with the credential names required by the repository's installer workflow.
- A build from a trusted release branch.

Store credential values only in the CI secret manager. Never commit certificates or private keys, paste them into chat, or print them in logs.

Updater signatures and Apple code signing are separate. A valid updater signature does not sign an app for macOS or notarize it.

## Build and verify

Run the repository's manual installer workflow with notarization enabled. The workflow should fail if credentials are missing rather than silently producing an unsigned package.

After building, verify the app signature and notarization ticket before collecting checksums:

~~~sh
codesign --verify --deep --strict --verbose=2 Dukto.app
xcrun stapler validate Dukto.app
spctl --assess --type execute --verbose Dukto.app
~~~

Also inspect the packaged app and updater archive to ensure they contain only the intended executables. The command-line client is distributed separately from the desktop bundle.

Label any explicitly unsigned evaluation package so that it is not mistaken for a notarized release. Signing the macOS application does not sign standalone CLI archives or Windows packages.

See the [official Tauri macOS distribution guide](https://v2.tauri.app/distribute/sign/macos/) for platform-specific details.
