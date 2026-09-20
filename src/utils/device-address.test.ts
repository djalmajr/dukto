import { describe, expect, test } from "bun:test";
import { selectPreferredIpv4 } from "./device-address";

describe("selectPreferredIpv4", () => {
	test("prefers Wi-Fi over an iOS USB link-local address", () => {
		expect(selectPreferredIpv4(["169.254.195.158", "192.168.0.6"])).toBe("192.168.0.6");
	});

	test("uses a link-local address only when no regular IPv4 address exists", () => {
		expect(selectPreferredIpv4(["fe80::1234", "169.254.195.158"])).toBe("169.254.195.158");
	});

	test("ignores loopback, unspecified and multicast addresses", () => {
		expect(selectPreferredIpv4(["0.0.0.0", "127.0.0.1", "224.0.0.251"])).toBeUndefined();
	});
});
