# Discovery

Dukto uses mDNS (multicast DNS) to find peers on the local network. No server, no manual IP entry, no pairing step.

## How It Works

On startup, the app registers itself as a `_dukto._tcp.local.` mDNS service and begins browsing for other instances of the same service. When another device is found, its information (name, hostname, platform, QUIC endpoint) is extracted from the mDNS TXT records and pushed to the frontend as an event.

Discovery is **push-based and continuous**. The frontend never polls — it subscribes to events and the UI updates reactively. Peers appear within a few seconds of coming online and disappear shortly after going offline.

## Self-Filtering

Each device includes its own `device_id` in the mDNS advertisement. When the browser finds a service, it checks the device_id and skips itself. This prevents the device from appearing in its own peer list.

## Why mDNS and Not Something Else

**Bonjour/Avahi/mDNS** is the only viable option for true zero-configuration LAN discovery:

- **UDP broadcast** would work but doesn't carry structured metadata (TXT records) and has scalability issues on large networks
- **SSDP/UPnP** is more complex, designed for service discovery rather than peer-to-peer, and has inconsistent implementations
- **Manual IP entry** defeats the purpose of zero configuration
- **Server-assisted discovery** requires infrastructure we don't want for the LAN case

mDNS is supported natively on macOS (Bonjour), and via Avahi on Linux. On Windows, the `mdns-sd` crate handles it without system dependencies.

## Advertised Metadata

Each device broadcasts a set of TXT properties alongside its mDNS service:

- **Protocol version** — allows future protocol changes without breaking discovery
- **Device ID** — stable UUID, generated once and persisted. Used for peer identification across sessions
- **Display name** — OS username, human-readable but not stable
- **Hostname** — machine hostname, also not stable
- **Platform** — "macos", "windows", or "linux". Used for platform-specific icons in the UI

The QUIC endpoint (IP addresses + port) comes from the mDNS service resolution, not from TXT records. mDNS automatically resolves the host to its network addresses.

## Event Channel

Discovery events are broadcast via a Tokio broadcast channel (capacity 64). The Tauri setup loop forwards these events to the frontend:

- **PeerFound** — upserts the peer in the shared state and emits `peer:found` to the frontend
- **PeerRemoved** — removes the peer from shared state and emits `peer:removed`

The broadcast channel means multiple consumers can subscribe (e.g., both the Tauri event forwarder and future background tasks).

## Limitations

- **Requires multicast support** — corporate networks that block multicast will prevent discovery
- **LAN only** — mDNS doesn't cross network boundaries. Internet discovery will need a signaling server (planned for a future phase)
- **Discovery latency** — peers typically appear within 2-5 seconds, but mDNS has no guaranteed SLA on timing
- **Network changes** — if the device changes networks (e.g., switches WiFi), the mDNS daemon re-advertises but peers on the old network may linger briefly
