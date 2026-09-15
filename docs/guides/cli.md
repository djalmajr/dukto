# Command-line transfers

`dukto` sends and receives files and folders using the same mDNS discovery,
QUIC transport, Noise encryption and receiver implementation as the desktop app.
It runs without a window or WebView. Install Rust to build it:

```sh
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin dukto-cli --release
```

The CLI archive contains `dukto` (`dukto.exe` on Windows). Extract it into its own directory and add that directory to PATH. Do not overwrite the desktop executable: it also uses this filename. If both are available on PATH, use the CLI's full path to avoid ambiguity.

Building from source produces `src-tauri/target/release/dukto-cli` (`dukto-cli.exe` on Windows). You can run that binary directly, or copy it to a separate directory as `dukto`. The release collector performs this rename. From the project root, `bun run cli --help` also works.

## Receive

```sh
dukto receive --destination ./received
```

The receiver advertises itself on the LAN and asks you to accept each transfer.
For scripts, explicitly authorize incoming transfers for this process:

```sh
dukto --json receive --destination ./received --accept --once
```

`--once` exits after one attempted transfer. Without it the receiver keeps running
until Ctrl+C. Without a terminal, `--accept` is required. It is never persisted.
Use a dedicated destination for unattended tests. The default port is 4242;
use `--port 4243` to run beside the desktop app, or `--port 0` for a dynamic port
reported in the `listening` event. The CLI has a separate persistent device ID
and does not change the desktop destination or settings.

## Concurrent transfers

A continuous receiver handles multiple QUIC connections concurrently. Start another
`send` command while an earlier one is still running, including to the same peer.
The same CLI identity can also keep `receive` running in another terminal while
sending. Each `send` process represents one transfer; a command with several paths
is one transfer containing several items.

```sh
# Terminal 1: keep receiving; approval is requested separately for each transfer.
dukto receive --destination ./received --port 4243

# Terminal 2: send a large file.
dukto send --peer DEVICE_ID -- ./large-a.bin

# Terminal 3: start another transfer without waiting for terminal 2.
dukto send --peer DEVICE_ID -- ./large-b.bin ./notes.txt
```

Approved transfers continue while later requests wait for approval. Interactive
prompts are queued and include the transfer ID; one answer applies only to the
printed request. Expired requests do not consume an answer intended for the next
prompt. An answer to a prompt that expired while waiting for input is discarded.
`--accept` approves every request for that receiver invocation, allowing unattended
concurrency tests; `--once` intentionally handles only one attempted transfer.

Progress, receipts and receive errors carry their transfer ID. Interrupting one
`send` process does not stop other send processes or a separate receiver. Ctrl+C
on a continuous receiver stops that receiver and its active connections. The
receiver reports individual connection failures and remains available for others.
Files with conflicting names are created exclusively and renamed, so concurrent
receipts cannot truncate an existing file. Existing folder roots may be shared;
conflicting files within them are independently renamed.

## Find and send

```sh
dukto --json peers --timeout 5
dukto send --peer DEVICE_ID -- ./file.txt
dukto send --peer DEVICE_ID -- ./a.txt ./b.txt
dukto send --peer DEVICE_ID -- ./folder
dukto send --peer DEVICE_ID -- ./folder-a ./folder-b
dukto send --peer DEVICE_ID -- ./a.txt ./b.txt ./folder-a ./folder-b
```

Files and folders can be mixed. Subdirectories, empty files and empty directories
are preserved; unsafe names are sanitized and existing files are renamed rather
than overwritten. Symlinks are skipped. Use `--` before paths beginning with `-`.

If multicast discovery is unavailable, supply the same QUIC endpoint directly:

```sh
dukto send --address 192.168.0.16:4242 -- ./file.txt
```

Use IPv4 endpoints (matching the desktop listener). Both machines must permit LAN
traffic: mDNS uses UDP 5353; QUIC uses the receiver's advertised UDP port. Remote
control of Codex does not establish LAN reachability for Dukto.

## Automation and completion

`--json` emits one JSON object per line, with `event` and `data` fields. Events
include `peers`, `listening`, `connecting`, `incoming`, `progress`, `sent`,
`received` and errors. Diagnostics use stderr. A successful send has
`acknowledged: true`; progress alone never establishes completion. Exit code 0
means success; failures, rejection, interruption and invalid arguments are nonzero.
Continuous receivers report individual failures as `receive_error` and continue.

`--timeout` on send is an overall deadline (default 300 seconds); on receive it
limits each active connection, not idle listening. `--data-dir` isolates identities
for tests, and `--name` sets the advertised display name for this invocation.

Protocol **0.2** requires a receiver receipt matching the transfer ID, item count
and bytes after writes are flushed. Update both CLI and desktop builds together:
old 0.1 apps cannot reliably interoperate with this completion handshake.
Partial files can remain after failed or interrupted transfers; resuming is not
implemented. The CLI uses the same ephemeral Noise identity behavior as the UI;
it does not add persistent trusted-device pairing.

## Verify

```sh
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features cli
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin dukto-cli
node scripts/test-cli.mjs
```

The process-level suite transfers a large binary, multiple files, a nested folder,
multiple root folders, and files mixed with folders in one transfer over
QUIC/Noise, then compares paths, sizes and SHA-256.
It also checks non-interactive approval and invalid input failures. These are
loopback tests; cross-machine and UI interoperability require separate validation.

Run the large-file concurrency regression separately:

```sh
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin dukto-cli --release
DUKTO_CLI=src-tauri/target/release/dukto-cli DUKTO_TEST_REPORT=.cache/concurrency-test/report.json node scripts/test-cli-concurrency.mjs
```

On Windows, set `DUKTO_CLI` to the `.exe` path using PowerShell environment variables.
The test generates three 512 MiB files, waits for real progress before adding more
transfers, requires overlapping progress and a later small transfer finishing first,
checks bidirectional traffic and SHA-256, then interrupts one sender while another
continues. Synthetic large files are removed after the run. `DUKTO_TEST_MIB` can
increase the size when a faster machine finishes too soon to observe overlap.
