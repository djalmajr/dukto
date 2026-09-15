# Desktop auto-updates

Dukto uses Tauri's updater plugin with the static feed at `https://github.com/djalmajr/dukto/releases/latest/download/latest.json`. The updater checks the feed for the installed platform, verifies the downloaded artifact with the embedded Minisign public key, and installs only an artifact signed by Dukto's updater key.

## Release artifacts

Run **Build installers (manual)** from `main`. The workflow builds these five desktop targets:

| Platform | First-install package | Signed updater package |
| --- | --- | --- |
| macOS Apple Silicon | DMG | `.app.tar.gz` and `.sig` |
| macOS Intel | DMG | `.app.tar.gz` and `.sig` |
| Windows x64 | NSIS setup `.exe` | setup `.exe` and `.sig` |
| Linux x64 | DEB and AppImage | AppImage and `.sig` |
| Linux arm64 | DEB and AppImage | AppImage and `.sig` |

The existing CLI archives remain separate and are not updater payloads. Each target also gets a `SHA256SUMS-<target>.txt` covering its release files.

The workflow requires the GitHub Actions secret `TAURI_SIGNING_PRIVATE_KEY` whenever `bundle.createUpdaterArtifacts` is enabled. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is optional. The key is passed to builds and, when needed, written to a mode-600 temporary file for Tauri's signer; it is never printed or passed as a command-line argument. Tauri's `--no-sign` option also skips automatic updater signatures. For evaluation builds, the workflow explicitly signs any missing updater bundle signature with the same Dukto updater key. This does not code-sign the installer for the operating system.

After all five target artifacts are downloaded, a dedicated job on every workflow run verifies the exact stable version and all checksums, then uses the Minisign verifier with the public key embedded in the app to cryptographically verify every updater bundle and signature pair. It fails if an asset, signature, checksum, version match, or signature verification is missing or invalid. The job writes `latest.json` with all five platform entries, pointing to assets on the same GitHub release and embedding the `.sig` contents. The validated JSON is uploaded separately as the `updater-feed` workflow artifact, including evaluation runs where no release draft is requested; it is kept separate from per-target checksum manifests. Evaluation runs run the same verifier tests using temporary Minisign key pairs, including tamper and wrong-key rejection cases.

The app checks for updates silently once on startup and reports an available version in an update dialog. Settings also offers a manual check and displays whether the app is current. The user chooses when to download; the dialog shows download progress and release notes. Before installation, Tauri verifies the downloaded update with the embedded public key. The user explicitly chooses “Install and restart” after download. If active transfer work prevents installation, the downloaded package remains available for the user to retry after that work ends. A pending incoming request takes visual priority over the update dialog; any available update is shown after the request is resolved.

## Review and publication

`create_release_draft` is false by default and can only be enabled from `main`. When enabled, CI creates a reviewable GitHub draft containing installers, updater bundles, CLI archives, checksum files and the already validated `latest.json`. CI does not publish the release. The feed builder rejects prerelease versions because this workflow produces the stable `/releases/latest/` feed. Review the draft and its assets before publishing it in GitHub; GitHub excludes drafts and prereleases from that URL. Publish a stable version matching `package.json` and `src-tauri/tauri.conf.json` for automatic updates to reach users.

## Key rotation

Keep the private key backed up securely. Replacing it without updating the public key embedded in the application breaks signature verification for existing installations. A signing-key rotation therefore requires a migration strategy and a new app build trusted by the old key before switching the release signer.
