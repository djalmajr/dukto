# Website and Release Builds

## Website

The website is a separate Solid/Vite entry point under site/. Its interface and navigation can be localized; documentation body content is maintained in US English in site/src/guides.ts.

Run these commands from the repository root:

~~~sh
bun run site:dev
bun run site:check
bun run site:build
bun run site:preview
~~~

These commands build or preview local files; they do not publish the site. Keep documentation links valid across the generated routes and verify the prerendered pages after editing the guide source.

Publish from `main` with the manual **Deploy website** GitHub Actions workflow (`.github/workflows/deploy-site.yml`). It builds and verifies the static pages before deploying the `dukto-site` Worker and its custom domain. Configure `CLOUDFLARE_API_TOKEN` as a secret and `CLOUDFLARE_ACCOUNT_ID` as a variable in the `website` environment, restricted to `main`. The production site is [dukto.app](https://dukto.app/).

## Installer workflow

The manual GitHub Actions workflow under .github/workflows builds desktop packages and CLI archives for Windows, macOS, and Linux targets. It creates per-target checksums and verifies updater signatures before producing the update feed.

Review the workflow definition for the current target matrix and required settings. Signing and notarization credentials must be provided as protected CI secrets; never commit secret values or put them in documentation, logs, or example configuration.

The workflow can create a reviewable draft release after its build and verification jobs pass. A draft is not a published release. Confirm version metadata, installer contents, checksums, signatures, and release notes before publishing. Only verified assets from a published stable release should be referenced by the download page or stable update feed.

## Local artifact collection

After a native Windows build, the release collector can gather the output:

~~~powershell
$env:DUKTO_RELEASE_DIR = 'src-tauri/target/release'
bun run release:collect x86_64-pc-windows-msvc
~~~

Collected artifacts remain under .cache/release/<target>. Inspect their checksums and contents before sharing them.

## Compatibility checks

Test the package on the target operating system and architecture. For macOS, verify signing and notarization separately; see the [macOS signing guide](macos-notarization.md). Linux compatibility depends on the distribution baseline and WebKitGTK version. A successful build on one Linux distribution does not establish compatibility with all distributions.

The CLI is distributed separately from the desktop app. Do not assume that signing or notarizing a desktop bundle applies to the CLI archives or Windows installers.
