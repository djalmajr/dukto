function parseIpv4(address: string): number[] | undefined {
	const octets = address.split(".").map(Number);
	if (
		octets.length !== 4 ||
		octets.some((octet) => !Number.isInteger(octet) || octet < 0 || octet > 255)
	) {
		return undefined;
	}
	return octets;
}

export function selectPreferredIpv4(addresses: string[]): string | undefined {
	let linkLocal: string | undefined;

	for (const address of addresses) {
		const octets = parseIpv4(address);
		if (!octets) continue;

		const [first, second] = octets;
		const unusable = first === 0 || first === 127 || (first >= 224 && first <= 239);
		if (unusable) continue;

		if (first === 169 && second === 254) {
			linkLocal ??= address;
		} else {
			return address;
		}
	}

	return linkLocal;
}
