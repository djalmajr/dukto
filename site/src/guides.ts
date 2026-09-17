export type Section = {
	title: string;
	text?: string;
	code?: string;
	items?: string[];
	link?: { label: string; href: string; external?: boolean };
};
export type Guide = {
	slug: string;
	navGroup: string;
	navTitle: string;
	group: string;
	title: string;
	summary: string;
	sections: Section[];
};
export const guides: Guide[] = [
	{
		slug: "primeiros-passos",
		navGroup: "COMECE AQUI",
		navTitle: "Primeiros passos",
		group: "GETTING STARTED",
		title: "Getting started",
		summary: "Two devices. One direct connection. Your files where they belong.",
		sections: [
			{
				title: "01. Open Dukto on both devices",
				text: "Use the same version on both devices and keep Dukto open so they can connect through the options available to them.",
			},
			{
				title: "02. Choose where to send",
				text: "Available devices appear through supported discovery or connection methods. Select a destination, then choose the files or folders you want to share.",
			},
			{
				title: "03. Approve on the receiving device",
				text: "Review the sender and the items before accepting. A transfer completes after the destination confirms receipt. Files are saved in the destination folder configured in the app.",
			},
			{
				title: "Prefer the terminal?",
				text: "Start a receiver on one device and discover its ID from the sender. The CLI uses the same transfer protocol as the app.",
				code: "# On the receiving device\ndukto receive --destination ./received\n\n# On the sending device\ndukto peers\ndukto send --peer DESTINATION_ID -- ./photo.jpg",
			},
		],
	},
	{
		slug: "instalacao",
		navGroup: "COMECE AQUI",
		navTitle: "Instalação",
		group: "GETTING STARTED",
		title: "Installation",
		summary: "Choose the app for everyday use or the CLI for your workflow.",
		sections: [
			{
				title: "Availability",
				text: "Check GitHub Releases for published installers and supported architectures. A download is available only when it appears in a release.",
				link: {
					label: "GitHub Releases",
					href: "https://github.com/djalmajr/dukto/releases",
					external: true,
				},
			},
			{
				title: "macOS",
				text: "Open the DMG and move Dukto to Applications. Choose the build for Apple Silicon or Intel. Local evaluation builds may not be signed or notarized by Apple.",
			},
			{
				title: "Windows",
				text: "Run the EXE installer and follow its steps. Allow access to private networks when Windows asks so devices can be discovered and receive files. Evaluation builds may not be signed with Authenticode.",
			},
			{
				title: "Linux",
				text: "Use the DEB package on compatible Debian or Ubuntu distributions. The AppImage is a portable alternative. After downloading it, make the file executable and launch it.",
				code: "chmod +x Dukto*.AppImage\n./Dukto*.AppImage",
			},
			{
				title: "Android",
				text: "When an Android build is published, use the package and installation instructions listed in GitHub Releases. Android may ask you to approve installation from the download source.",
			},
			{
				title: "iOS",
				text: "When an iOS build is published, use the distribution method listed in GitHub Releases. Follow the release instructions for TestFlight or a signed build.",
			},
			{
				title: "Use the dukto command",
				text: "The CLI archive contains dukto (dukto.exe on Windows). Extract it into its own folder and add that folder to PATH. Do not replace the desktop app executable, which uses the same name. Source builds produce dukto-cli; run it directly or copy it to a separate folder as dukto.",
			},
			{
				title: "Build the CLI",
				text: "Install Rust and the build tools for your platform, then run this command from the repository root. The CLI does not require a WebView.",
				code: "cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features cli --bin dukto-cli --release\n\n# The binary is written to src-tauri/target/release/",
			},
		],
	},
	{
		slug: "arquivos-e-pastas",
		navGroup: "USANDO O DUKTO",
		navTitle: "Arquivos e pastas",
		group: "USING DUKTO",
		title: "Files and folders",
		summary: "Send a single document or a whole folder while keeping its structure.",
		sections: [
			{
				title: "One or more items",
				text: "The CLI accepts files, folders, and mixed selections in one command. Folders are traversed recursively, including subfolders and empty directories.",
				code: "dukto send --peer ID -- ./report.pdf\ndukto send --peer ID -- ./a.txt ./b.txt\ndukto send --peer ID -- ./project\ndukto send --peer ID -- ./photos ./documents\ndukto send --peer ID -- ./note.txt ./photos ./documents",
			},
			{
				title: "Names and conflicts",
				text: "Existing files are kept. New files use an available conflict name rather than overwriting them. Invalid names are normalized for cross-platform compatibility, and symbolic links are skipped.",
			},
			{
				title: "If a transfer is canceled or fails",
				text: "Incomplete files are received into a temporary file and are removed if the transfer ends before completion. Fully received files remain in place. Dukto does not resume interrupted transfers; send the items again and verify the receiver's completion status.",
			},
		],
	},
	{
		slug: "cli",
		navGroup: "USANDO O DUKTO",
		navTitle: "Referência da CLI",
		group: "USING DUKTO",
		title: "CLI reference",
		summary: "Use the same direct transfer engine with commands that fit your scripts.",
		sections: [
			{
				title: "Discover devices",
				code: "dukto --json peers --timeout 10",
				text: "The output lists the ID, name, platform, addresses, port, and protocol version of receivers discovered through mDNS.",
			},
			{
				title: "Receive",
				code: "dukto --name Studio receive --destination ./received --port 4243",
				text: "By default, the CLI asks for approval for each incoming transfer. Use a different port from the desktop app when both run on the same computer.",
			},
			{
				title: "Explicit automation",
				code: "dukto --json receive --destination ./received --port 4243 --accept --once",
				text: "--accept authorizes incoming transfers for this process only. --once exits after one attempt. Without an interactive terminal, --accept is required. Use a dedicated destination folder for tests.",
			},
			{
				title: "Send by ID or address",
				code: "dukto send --peer ID -- ./folder\n\n# Use an address if discovery is unavailable\ndukto send --address 192.168.0.16:4243 -- ./folder",
				text: "Use an IPv4 address. --timeout sets the total send deadline in seconds (default: 300). On a receiver, it limits each active connection, not idle wait time.",
			},
			{
				title: "Read the result",
				text: "--json emits one JSON event per line. A sent event includes acknowledged: true only after the receiver confirms completion. Exit code 0 means success; failures return a nonzero code. A continuous receiver reports receive_error and continues. Use --data-dir to isolate device identity data.",
			},
		],
	},
	{
		slug: "rede-e-descoberta",
		navGroup: "ENTENDA O FLUXO",
		navTitle: "Rede e descoberta",
		group: "UNDERSTAND THE FLOW",
		title: "Network and discovery",
		summary:
			"Dukto connects devices directly, using local discovery when they share a network and an address when needed.",
		sections: [
			{
				title: "Connection and discovery",
				text: "Discovery uses mDNS on UDP port 5353. Transfers use QUIC on the UDP port announced by the receiver (4242 by default). Guest networks, Wi-Fi isolation, VPNs, or firewalls may prevent devices from finding each other.",
			},
			{
				title: "A device does not appear",
				items: [
					"Keep the receiver open and confirm both devices use compatible protocol versions.",
					"Confirm the devices are not on isolated or guest networks.",
					"Check that the firewall allows mDNS and the receiver's UDP port.",
					"Use --address in the CLI to separate discovery problems from transfer problems.",
				],
			},
			{
				title: "Virtual machines and WSL",
				text: "Virtual network adapters may block multicast or make a guest unreachable from other devices. Configure the VM or WSL networking mode and firewall to allow mDNS and the receiver's UDP port. If discovery is unavailable, use the receiver address with the CLI.",
			},
			{
				title: "Each environment has its own identity",
				text: "Separate operating-system environments have separate Dukto identities, even when they share a hostname or IP address. Use a distinct receiver port when two instances run on the same machine.",
			},
		],
	},
	{
		slug: "privacidade",
		navGroup: "ENTENDA O FLUXO",
		navTitle: "Privacidade",
		group: "UNDERSTAND THE FLOW",
		title: "Privacy and security",
		summary: "Files travel directly between devices. You decide what to receive.",
		sections: [
			{
				title: "Direct, encrypted transfer",
				text: "Content travels over QUIC with a Noise session. Dukto's direct transfer flow does not send files to a cloud storage service.",
			},
			{
				title: "Approval and trust",
				text: "The graphical app asks you to approve each incoming transfer. The CLI requires explicit --accept for automatic receiving. Dukto does not currently provide persistent pairing or a trusted-device list. Display names and advertised IDs are self-reported labels, not proof of a person's identity.",
			},
			{
				title: "Protocol compatibility",
				text: "Protocol 0.2 requires a receipt containing the transfer ID, item count, and byte count. Update the sender and receiver together. Older applications using protocol 0.1 are not compatible with this receipt.",
			},
		],
	},
	{
		slug: "validacao",
		navGroup: "ENTENDA O FLUXO",
		navTitle: "Solução de problemas",
		group: "UNDERSTAND THE FLOW",
		title: "Troubleshooting",
		summary:
			"Check network access, protocol compatibility, and transfer status when something goes wrong.",
		sections: [
			{
				title: "Discovery and connection",
				items: [
					"Confirm both devices are connected to a network that allows local multicast.",
					"Allow UDP 5353 for mDNS discovery and the receiver's announced UDP port for QUIC transfers.",
					"Use the CLI --address option to test a transfer without relying on discovery.",
				],
			},
			{
				title: "Transfer declined or failed",
				text: "The receiver must approve each incoming transfer. Check the receiver's transfer card or CLI output for the final result. A successful sender result requires a completion receipt from the receiver.",
			},
			{
				title: "Canceled transfers",
				text: "Canceling a transfer removes its incomplete staged file at the destination. Files that were fully received before cancellation and files that existed before the transfer are preserved. Dukto does not resume an interrupted transfer.",
			},
		],
	},
];
