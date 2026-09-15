# macOS signing and notarization

Dukto follows Markdraw's `.github/workflows/build-desktop.yml`: a Developer ID Application certificate signs the app, and an App Store Connect API key authenticates Apple notarization. Tauri performs signing, submission and stapling during its normal build. The reference was inspected locally on 2026-09-14; no Markdraw files or credentials were modified.

## Required environment secrets

Configure these in [Dukto release environment](https://github.com/djalmajr/dukto/settings/environments), using the original signing materials. The same names exist in Markdraw. GitHub exposes their names but does not return their decrypted values, so they cannot be copied from one repository with `gh secret list`.

| Secret | Value |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64-encoded Developer ID Application certificate and private key exported as `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | Password protecting that `.p12` |
| `APPLE_SIGNING_IDENTITY` | Full Developer ID Application signing identity |
| `APPLE_API_ISSUER` | App Store Connect team API issuer ID |
| `APPLE_API_KEY` | App Store Connect API key ID |
| `APPLE_API_KEY_P8_BASE64` | Base64-encoded private `.p8` key |

Use the same Apple team and suitable credentials as Markdraw. Do not commit these values or paste them into logs or chat. `TAURI_SIGNING_PRIVATE_KEY` signs Dukto's updater artifacts on every build platform; it is separate from these Apple credentials and does not notarize the macOS app. See [desktop auto-updates](updater.md) for the updater key and release flow.

## Run and verify

Run **Build installers (manual)** from `main`. `notarize_macos` defaults to true and applies to both Apple Silicon and Intel builds. The workflow validates all six secrets, decodes the `.p8` only into a private runner temporary file, and removes it even if the build fails. A requested notarized build fails if credentials are absent; it does not fall back to an unsigned artifact.

After Tauri builds the DMG and updater archive, CI checks the bundled `Dukto.app` using `codesign --verify`, `xcrun stapler validate` and `spctl --assess`. Collection and checksums happen only after those checks succeed. The app's identifier remains `com.dukto.app`.

Set `notarize_macos=false` only for an explicit unsigned evaluation build. Draft release notes record the selected mode. `create_release_draft` remains false by default; enabling it only creates a draft, never a public release. No installer workflow or Apple submission is triggered by a PR or push.

The separate CLI archives are not signed or notarized by this desktop flow. Windows Authenticode is also not configured. Existing evaluation artifacts remain unsigned; merging this workflow does not retroactively notarize them.

## Current verification

The six Apple secrets are configured in the `release` environment from the existing Markdraw signing backups. The certificate was checked for expiry and the signing identity was extracted directly from the certificate to preserve its Unicode spelling. Markdraw credentials and backup files were not changed.

Initial signed run [34936943117](https://github.com/djalmajr/dukto/actions/runs/34936943117) passed Apple Silicon signing, stapling and Gatekeeper verification. Intel exposed an extra unsigned `dukto-cli` inside the desktop bundle. The CLI now requires the explicit `cli` feature and its entry point lives outside `src/bin`, which the current Tauri CLI scans independently. Desktop packages contain only `dukto`; the standalone CLI is built separately. CI checks both the app directory and updater archive for this separation.

Reference: [Tauri macOS signing and notarization](https://v2.tauri.app/distribute/sign/macos/).

Signing secrets belong to the `release` environment, configured with a selected deployment branch policy allowing only `main`. Do not duplicate them as repository secrets: a workflow on another branch could otherwise access them. The build job uses this environment, so the policy also covers signing when no release draft is requested.
