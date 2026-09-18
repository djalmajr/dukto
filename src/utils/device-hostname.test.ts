import { describe, expect, test } from "bun:test";
import { formatDeviceHostname } from "./device-hostname";

describe("formatDeviceHostname", () => {
	test("hides the mDNS local suffix from user-visible device names", () => {
		expect(formatDeviceHostname("Run2Biz.local")).toBe("Run2Biz");
		expect(formatDeviceHostname("sm-g781b.LOCAL.")).toBe("sm-g781b");
	});

	test("keeps names whose local text is not an mDNS suffix", () => {
		expect(formatDeviceHostname("local-device")).toBe("local-device");
		expect(formatDeviceHostname("example.localdomain")).toBe("example.localdomain");
	});
});
