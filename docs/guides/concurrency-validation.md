# Concurrent Transfer Testing

This guide describes how to verify overlap between real transfers. Starting two processes at nearly the same time or seeing two progress bars is not enough: the second transfer must make progress before the first one finishes.

## CLI regression

Build the CLI with release optimizations, then run the concurrency test from the repository root:

~~~sh
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin dukto-cli --release
DUKTO_CLI=src-tauri/target/release/dukto-cli DUKTO_TEST_REPORT=.cache/concurrency-test/report.json node scripts/test-cli-concurrency.mjs
~~~

On Windows, point DUKTO_CLI to the compiled .exe using the shell's environment-variable syntax. The test creates synthetic large files, waits for actual progress before adding more transfers, checks overlapping progress and a later small transfer, exercises both directions, compares receiver acknowledgments and SHA-256 values, and interrupts one sender while another continues. The test removes its synthetic files after completion. DUKTO_TEST_MIB can increase the file size when transfers finish too quickly to observe overlap.

## What to verify

- Start a large transfer and wait until progress is above zero and below 100 percent.
- Start a second large transfer to the same receiver while the first is active.
- Start a small transfer and confirm it can finish before the large transfers.
- Exercise a receive in the reverse direction while sends remain active.
- Confirm each result is associated with its own transfer ID and each successful send has a matching receiver acknowledgment.
- Compare hashes for every completed file.
- Cancel one sender and confirm other transfers and the receiver remain usable.
- Repeat with identical filenames and different contents to check collision handling.

Record elapsed times relative to each test process; do not compare cross-host wall clocks unless they are synchronized.

## Desktop UI

CLI results do not prove native UI behavior. Follow the [manual UI test guide](manual-ui-tests.md#concurrent-transfers) to verify multiple progress rows, per-transfer cancel actions, and simultaneous send and receive in the desktop app.

Use a dedicated destination for each run. Remove only the synthetic fixtures created by the test, and retain concise manifests or test reports when needed for a regression.
