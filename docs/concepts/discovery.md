# Discovery

Dukto uses multicast DNS (mDNS) to find peers on the local network. No account or manual address entry is needed when multicast is available.

## How It Works

Each running instance advertises a `_dukto._tcp.local.` DNS-SD service and browses for other Dukto instances. The advertisement includes the device ID, display name, host name, platform, protocol version, and the UDP port used for QUIC. The resolved mDNS record supplies network addresses.

The app continuously updates the host list as devices appear and disappear. An instance filters its own advertisement so it does not show itself as a peer.

## Network Requirements

- Devices must be able to exchange mDNS multicast traffic on UDP port 5353.
- Firewalls must allow UDP traffic to the receiver's advertised QUIC port (4242 by default).
- Guest Wi-Fi, client isolation, VPN configuration, and virtual-machine networking can block discovery or direct transfers.
- mDNS does not cross routers by default. Dukto's current transfer flow is for devices on a local network.

If discovery is unavailable, the CLI can connect to a receiver by address. A direct-address transfer does not validate that mDNS discovery is working.

## Identity and Advertised Information

The device ID is generated and stored locally to distinguish Dukto instances between sessions. Display names and host names help people recognize peers, but they can change and are not verified identity credentials. A peer can advertise arbitrary labels.

The receiver address and UDP port are resolved through the service record. The service type identifies Dukto on the network; it does not authenticate the device.

## Troubleshooting

- Keep Dukto open on both devices and use compatible protocol versions.
- Confirm both devices can use the same non-isolated local network.
- Allow mDNS on UDP 5353 and the receiver's advertised UDP port in the firewall.
- Try a direct CLI address to distinguish discovery problems from connection problems.
- When using WSL or a virtual machine, check its network mode and host firewall rules for multicast and UDP access.
