# Installer and updater validation — 2026-09-14–15

## Scope and isolation

Production configuration uses a dedicated Dukto public updater key and the HTTPS GitHub release feed. Native tests used temporary configuration files, an isolated application identifier (`com.dukto.updater-test`), versions 0.1.0/0.1.1, and a loopback feed. Test versions and HTTP exceptions are not part of production configuration.

The production app was closed while the isolated transfer service used its port, then reopened. User data and the original bundle were preserved. No public release was published.

## Automated checks

- 52 JavaScript tests passed (523 assertions), including controller transitions, release collection, and real Minisign verification of manifest assets.
- Desktop Rust library: 60 tests passed.
- CLI/core without desktop features: 56 tests passed across library, CLI, concurrency, and end-to-end targets.
- Atomic transfer/update exclusion: four focused tests passed. Removing the registration guard caused both updater tests to fail; restoring it returned all four to passing.
- Lint, TypeScript, site type checking, Cargo formatting, and release compilation passed.

## Native macOS

The isolated app was installed from a mounted DMG. Native UI actions exercised:

1. Missing feed: startup remained quiet; manual checking showed an actionable error.
2. Corrupted archive with an unchanged signature: download failed signature verification and installation was unavailable; the installed version remained 0.1.0.
3. Valid signed archive: download completed with byte progress and explicit install/restart controls.
4. Incoming 8 GiB transfer while downloading: the receiving prompt took priority, displayed sender identity, and download continued in the background.
5. Installation during the accepted transfer: backend returned the busy state without restarting or discarding the downloaded update. The transfer continued and completed with receiver acknowledgement.
6. Source and destination were both 8,589,934,592 bytes with SHA-256 `ebfb4ef19ae410f190327b5ebd312711263bc7579970e87d9c1e2d84e06b3c25`. Both synthetic files were then removed.
7. Retrying installation after transfer completion updated the bundle to 0.1.1 without another package request. A separate repeat confirmed the relaunched process existed before any UI automation selected the app.
8. Settings/update dialog stacking and Settings overlay dismissal worked.
9. Resize from 600×600 to 480×360 exposed stale WebKit viewport-unit calculations. The updater now constrains its height by percentage of the containing block. Long notes scroll while Portuguese headings, progress, and buttons remain visible. Maximum width 640 was also checked.

Local evidence is under `.cache/updater/native/`: transfer logs/hashes, build logs, and screenshots `01` through `08`. Screenshot `06` records the original layout defect; `08-update-minimum-size-fixed.png` records its correction. Diagnostic instrumentation was removed from source.

The macOS DMG passed `hdiutil verify`, and the updater archive signature was independently verified against the configured public key. The production collector generated DMG, updater archive/signature, CLI archive, and checksums.

## Windows and release limitations

The connected Windows session built isolated NSIS installers for 0.1.0 and 0.1.1 with updater signatures. Its frontend checks and four release-mode transfer/update exclusion tests passed. Native Windows interaction remains pending because the session is locked (black capture and activation failure). Installer execution, update installation/relaunch, and UI behavior on Windows are not recorded as passed.

The manual workflow builds five platform targets and verifies every updater signature before assembling `latest.json`. Evaluation artifacts do not constitute a public release. The Apple signing secrets were subsequently provisioned from the Markdraw backups (see the current notarization guide); the updater signature is separate and does not replace Apple code signing/notarization.

## Hosted installer evaluation

The first main-branch evaluation run (`34926352630`) exposed an environment dependency before packaging: both hosted macOS runners failed four multicast discovery tests, including resolving their own raw mDNS announcement. Only IPv6 interfaces appeared in that diagnostic. The macOS installer job now excludes the five LAN-dependent discovery tests (including the negative self-discovery test, which cannot be meaningful without working discovery). It still runs the remaining core/CLI/end-to-end tests. Full discovery coverage remains required on Linux/Windows in the matrix and passed in the local macOS lab. No production discovery code or test assertions were weakened.
