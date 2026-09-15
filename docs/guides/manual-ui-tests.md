# Manual Desktop UI Tests

Use two computers with current Dukto builds on the same local network. Test real file dialogs and native drag and drop; CLI or browser-only checks do not replace desktop testing.

## Prepare

- Avoid running duplicate desktop instances on one computer.
- Choose an empty destination folder on each receiver and note the current destination so it can be restored.
- Confirm that the app is allowed through the local firewall for mDNS and the receiver's advertised UDP port.
- Use the same compatible app version on both computers.

## Discovery

- Confirm each app lists the other without using a direct address.
- Close one app and confirm the peer disappears after discovery updates.
- Reopen it and confirm the peer returns.
- Record discovery and transfer results separately. A direct-address send is not discovery success.

## Fixture set

Prepare equivalent fixtures on both computers:

| Case | Selection |
| --- | --- |
| A | One small file |
| B | Several files, including a Unicode filename, a filename with spaces, and an empty file |
| C | One folder containing a nested folder and an empty folder |
| D | Several root folders, including an empty folder |
| E | A file and a folder selected together |

Repeat each case in both directions.

## UI-to-UI transfer

For each direction and case:

1. Set a unique, empty destination folder on the receiver.
2. Select the destination host's ellipsis menu and choose Add files or Add folders, or drop items directly onto that host card.
3. Confirm the preview shows the intended host, names, item count, and size.
4. Send the files. On the receiver, inspect the sender and items, then accept.
5. Confirm progress and completion on both sides. The sender should receive an acknowledgment.
6. Verify filenames, relative folder structure, empty files, and empty folders.
7. Compare SHA-256 for every file and record the result.

Repeat single- and multi-file selection using native drag and drop on both operating systems. A picker test does not validate native file dragging.

## UI and CLI interoperability

Build the CLI from the repository root:

~~~sh
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin dukto-cli --release
~~~

The build output is named dukto-cli (dukto-cli.exe on Windows). Keep it separate from the desktop executable. For UI-to-CLI, start a CLI receiver on an unused port, choose a dedicated destination, then approve the transfer in the terminal. For CLI-to-UI, keep the desktop app open as the receiver and accept the incoming request in the app.

Use peers to obtain the current receiver ID. If discovery is unavailable, test a direct address separately and record it as a transfer test rather than a discovery test. A successful JSON send exits with status 0 and includes acknowledged: true.

## Host-specific draft behavior

- Add items to one host, open another host's menu, and confirm the two drafts stay separate.
- Open file and folder pickers repeatedly. New selections should accumulate in the same host draft.
- Cancel a picker and confirm the existing draft remains.
- Add a duplicate path and confirm it appears only once.
- Remove an item, add it again, and verify the count and size.
- Mix picker selection and drag and drop.
- Cancel a preview and start a fresh selection. Old items must not return.
- Make a host disappear while its draft is open. Its items must not move to another host.
- Drop over each card and over empty space. Only the card under the pointer may receive the drop.

## Window and ordering

- Drag the host header to reorder the list. Confirm the list swaps positions as the preview passes other hosts.
- Drop to save the order; press Escape to restore the prior order.
- Confirm progress rows, menus, and drafts remain attached to the correct host.
- Test at widths of 480, 600, and 640 pixels. Check long names, menus, and previews without horizontal scrolling.
- At the minimum window height of 360 pixels, reach every card and action with vertical scrolling and keyboard navigation.

## Concurrent transfers

For native concurrency checks, use large synthetic files and a dedicated destination. Follow the [concurrency testing guide](concurrency-validation.md): wait for real progress before starting another transfer, verify overlap, cancel one transfer, and check acknowledgments and hashes.

## Record results and clean up

For each failure, record the direction, case, sender and receiver, expected and observed behavior, exact error, and screenshots. Identify whether it failed during discovery, selection, preview, approval, transfer, or integrity checking.

Stop temporary CLI receivers with Ctrl+C, restore original destination settings, and remove only the synthetic files created for the test. Preserve manifests and logs until reviewed. Never recursively delete a directory that may contain personal files.
