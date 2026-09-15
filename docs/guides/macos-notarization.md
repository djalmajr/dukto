# macOS signing and notarization

Dukto follows Markdraw's `.github/workflows/build-desktop.yml`: a Developer ID Application certificate signs the app, and an App Store Connect API key authenticates Apple notarization. Tauri performs signing, submission and stapling during its normal build. The reference was inspected locally on 2026-09-14; no Markdraw files or credentials were modified.

## Required repository secrets

Configure these in [Dukto Actions secrets](https://github.com/djalmajr/dukto/settings/secrets/actions), using the original signing materials. The same names exist in Markdraw. GitHub exposes their names but does not return their decrypted values, so they cannot be copied from one repository with `gh secret list`.

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

The workflow structure, shell syntax and missing-secret failure path were checked locally. The six Apple secrets were absent from Dukto when inspected, so no signed build or Apple acceptance has been claimed. After provisioning the secrets, run the manual workflow and require both macOS verification steps to pass before distributing the artifacts as notarized.

Reference: [Tauri macOS signing and notarization](https://v2.tauri.app/distribute/sign/macos/).
