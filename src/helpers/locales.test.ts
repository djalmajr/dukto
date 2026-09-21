import { expect, test } from "bun:test";
import { createInstance } from "i18next";
import en from "../routes/locales/en.json";
import es from "../routes/locales/es.json";
import pt from "../routes/locales/pt.json";
import { nativeErrorKey } from "./native-error";

test("all UI locales have matching keys and interpolation parameters", () => {
	for (const locale of [pt, es]) {
		expect(Object.keys(locale).sort()).toEqual(Object.keys(en).sort());
		for (const key of Object.keys(en) as Array<keyof typeof en>) {
			expect(locale[key].match(/{{\w+}}/g)?.sort() ?? []).toEqual(
				en[key].match(/{{\w+}}/g)?.sort() ?? [],
			);
		}
	}
});

test("errors and plural item counts follow the selected language", async () => {
	const i18n = createInstance();
	await i18n.init({
		lng: "pt",
		resources: { en: { translation: en }, pt: { translation: pt }, es: { translation: es } },
	});
	expect(i18n.t(nativeErrorKey("connection lost"))).toBe("Conexão perdida.");
	expect(i18n.t("items", { count: 1 })).toBe("1 item");
	expect(i18n.t("items", { count: 2 })).toBe("2 itens");
	await i18n.changeLanguage("es");
	expect(i18n.t(nativeErrorKey("connection lost"))).toBe("Conexión perdida.");
	expect(i18n.t("items", { count: 2 })).toBe("2 elementos");
	await i18n.changeLanguage("en");
	expect(i18n.t(nativeErrorKey("connection lost"))).toBe("Connection lost.");
});

test("internet transfer copy is localized and compact at the stacked breakpoint", () => {
	expect(pt.internetTransferTitle).toBe("Transferir pela internet");
	expect(es.remoteStatusReadyRelay).toContain("relay");
	expect(en.remoteStatusReadyDirect).toBe("Connected directly");

	const actionKeys = [
		"createInternetInvitation",
		"connectWithInvitation",
		"copyInvitationLink",
		"cancelInvitation",
		"disconnectInternetSession",
		"confirmPairingCode",
	] as const;
	for (const locale of [en, pt, es]) {
		for (const key of actionKeys) expect(locale[key].length).toBeLessThanOrEqual(24);
	}
});
