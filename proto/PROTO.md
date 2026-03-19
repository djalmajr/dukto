# Proto

## Stack

- **Framework:** SolidJS (same as app)
- **Styling:** Tailwind CSS v4 via `@tailwindcss/vite`
- **Icons:** Iconify CDN (`iconify-icon` web component) — real app uses `unplugin-icons`
- **Dev server:** Vite standalone on port 3333
- **Run:** `bun run proto:dev` from project root

## Project Structure

```text
proto/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── index.html          # includes Iconify CDN script
└── src/
    ├── index.tsx        # render root, passes props to WindowFrame + AppContent
    ├── styles.css       # @import "tailwindcss"
    ├── stores/
    │   └── app.ts       # mock data, state machine, theme, sortFiles()
    ├── screens/
    │   └── AppContent.tsx  # wires mock state to real + proto components
    └── components/      # proto-only components (WindowFrame, Icon, etc.)
```

## Vite Alias

Proto imports real app components from `../src/` via relative paths. If paths get deep, add alias:

```ts
resolve: {
  alias: { "@app": path.resolve(__dirname, "../src") }
}
```

## Available App Components (pure, props-based)

| Component | Path | Props |
| --- | --- | --- |
| `EmptyState` | `src/components/EmptyState.tsx` | `title`, `description?` |
| `ErrorDisplay` | `src/components/ErrorDisplay.tsx` | `message`, `onDismiss?` |
| `PeerCard` | `src/features/peers/PeerCard.tsx` | `peer`, `onSelect?` |
| `PeerList` | `src/features/peers/PeerList.tsx` | `onPeerSelect?` |
| `SendPreview` | `src/features/transfers/SendPreview.tsx` | `files`, `onConfirm`, `onCancel`, `onRemoveFile?` |
| `TransferProgress` | `src/features/transfers/TransferProgress.tsx` | reads from `transfers` store |
| `IncomingRequest` | `src/features/transfers/IncomingRequest.tsx` | reads from `transfers` store |
| `PeerSelector` | `src/features/transfers/PeerSelector.tsx` | `onSelect`, `onCancel` |
| `DestinationFolder` | `src/features/settings/DestinationFolder.tsx` | reads from `settings` store |

**Note:** Some components still import stores directly (PeerList, TransferProgress, IncomingRequest, DestinationFolder). These need refactoring to pure props before importing in the proto. Until then, the proto has its own versions.

## Proto-Only Components

| Component | Purpose |
| --- | --- |
| `WindowFrame` | macOS window chrome with traffic lights, device info in titlebar, theme select, settings gear |
| `Icon` | Thin wrapper around `<iconify-icon>` web component |
| `PeerCard` | Proto version with `resolvedTheme()` for dark mode (app version uses `dark:` classes) |
| `TransferBar` | Progress bar with direction, status, bytes, speed, dismiss |
| `Button` | Styled button with `primary`/`secondary` variants |

## Mock Data

`proto/src/stores/app.ts` contains:

- `MOCK_PEERS` — 3 peers (macOS, Windows, Linux)
- `MOCK_FILES` — 10 files/folders of varying sizes
- `sortFiles()` — directories first (A-Z), then files (A-Z)
- `AppScreen` union type with all states
- Theme management (light/dark/system)

## Screen States

| State | Description | Triggered by |
| --- | --- | --- |
| `idle` | No activity, shows peer list or empty state | Default / Reset |
| `preview` | Peer header + sorted file list with remove buttons | Click peer |
| `sending` | Progress bar with animated percent | Click Send in preview |
| `incoming` | Modal: accept/reject with item count and sender | Proto control |
| `receiving` | Progress bar with animated percent | Accept incoming |
| `complete` | Green progress bar + dismiss button | Transfer finishes |
| `error` | Red banner with dismiss | Proto control |

## Flow

```
Click peer → preview (peer as header, files sorted, removable)
         → Send → sending (animated) → complete
         
Incoming → accept → receiving (animated) → complete
        → reject → idle
```

No "Send files..." / "Send folder..." buttons. No "Select recipient" modal. All actions start from a peer.
