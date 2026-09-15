# Website and installers

The website is a separate Solid/Vite entrypoint in `site/`, sharing the desktop's button primitive. `site/src/guides.ts` is the website documentation source. The production build prerenders HTML for all documentation routes, downloads, the homepage and 404, with canonical URLs and a sitemap for https://dukto.djalmajr.dev. No service or account is needed to run the preview.

```sh
bun run site:dev
bun run site:check
bun run site:build
bun run site:preview
```

The review server uses `127.0.0.1:4178`; production preview uses `127.0.0.1:4179`. Nothing is published by these commands. `site/wrangler.jsonc` prepares a Cloudflare static-assets Worker on the requested custom domain. After explicit approval and with the correct Cloudflare account selected, deployment is `bunx wrangler deploy --config site/wrangler.jsonc` from the repository root. DNS/custom-domain changes are part of publication and have not been performed.

## GitHub releases

The source repository `djalmajr/dukto` is public (verified on 2026-09-14), and no published releases were present at that check. A published release in this repository can provide public downloads. The website keeps the download state marked as in preparation until release assets exist and their URLs and checksums have been verified.

`.github/workflows/build-installers.yml` is manual only. It builds desktop installers and CLI archives for Windows x64, macOS arm64/x64, and Linux x64/arm64, with per-platform SHA-256 manifests. Every run validates checksums and cryptographically verifies all five updater signatures against the public key embedded in the app, then uploads a separate `updater-feed` artifact containing `latest.json`, even when `create_release_draft` is false. Optional `create_release_draft` is restricted to `main`, creates a reviewable draft after all five builds pass, reuses that validated feed and never publishes a release. Configure required reviewers for the `release` environment before using that input. The workflow has not been run as part of the local preview.

The desktop app checks the stable GitHub Releases feed at `https://github.com/djalmajr/dukto/releases/latest/download/latest.json`. CI requires the `TAURI_SIGNING_PRIVATE_KEY` secret in the `release` GitHub environment (restricted to `main`) when updater artifacts are enabled; the optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is empty for the current key. Tauri's `--no-sign` skips platform code signing and automatic updater signatures; the workflow signs missing updater payloads explicitly, and the feed job verifies them against the embedded public key. Drafts and prereleases are not served by GitHub's `/releases/latest/` URL. See [desktop auto-updates](updater.md) for the artifact mapping, verification and publication flow.

Local collection after a native build:

```powershell
$env:DUKTO_RELEASE_DIR = 'src-tauri/target/release'
bun run release:collect x86_64-pc-windows-msvc
```

Artifacts stay in `.cache/release/<target>`. Publishing a draft is a separate, user-approved step in GitHub. Only after publication should the download page be updated with verified asset URLs under `https://github.com/djalmajr/dukto/releases/download/<tag>/`. It intentionally presents release availability truthfully during preview, without fabricated download URLs.

## Signing and compatibility

Existing evaluation installers are unsigned. The manual installer workflow supports Developer ID signing and Apple notarization for both macOS architectures, enabled by default and requiring the six Apple secrets. It validates the stapled app before collecting artifacts. Explicit evaluation runs can disable notarization; draft release notes record the mode. See [macOS signing setup](macos-notarization.md). The separate CLI archives and Windows installers remain unsigned for operating-system code signing. No signed release has been produced merely by adding this configuration.

Linux arm64 artifacts built locally in Ubuntu 26.04 do not establish compatibility with older distributions. CI uses an older supported WebKitGTK 4.1 baseline. The dedicated Linux VM and Mac hold their local build artifacts; the Windows report under `.cache/cli-lan/mac-report.json` contains paths and checksums.

## Website identity, languages, and theme

The site imports the official `assets/brand/dukto-icon.svg` and uses the shared Button variants and sizes, with the website color palette. Platform and preference icons are compiled by unplugin-icons.

Portuguese (pt-br) is at `/`, English (en-us) at `/en-us/`, and Spanish (es-es) at `/es-es/`. Each language includes the homepage, downloads, all seven guides, the docs index, and a 404 page: 33 prerendered pages. The selector keeps the current page and anchor. Internal links, HTML language, titles, descriptions, and alternate-language metadata follow the selected locale. Translation dictionaries are in `site/src/locales/`, keyed by the Portuguese source text; command syntax and example filenames remain executable as written.

The light/dark toggle starts with the operating system preference, then saves an explicit choice under `dukto-site-theme` in localStorage. A head script applies that preference before the page renders. The site still works when browser storage is unavailable.

The site now imports the desktop solid-ui theme tokens directly for both light and dark modes. Its blue editorial accents follow the logo; backgrounds, foregrounds, borders, button colors and radii follow the app. Both data-theme and data-kb-theme are synchronized before rendering and when toggling.
