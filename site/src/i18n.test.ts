import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import ts from "typescript";
import { guides } from "./guides";
import { locales, localizedPath, routeLocale, stripLocale, translate } from "./i18n";
import en from "./locales/en-us.json";
import es from "./locales/es-es.json";
import guideEs from "./locales/guides.es.json";
import guidePt from "./locales/guides.pt.json";
import { pageMetadata } from "./metadata";

describe("website localization", () => {
	test("English is canonical and language switching preserves paths and anchors", () => {
		const path = "/pt/docs/cli/#section-2";
		expect(locales).toEqual(["en", "es", "pt"]);
		expect(localizedPath("en", path)).toBe("/docs/cli/#section-2");
		expect(localizedPath("es", path)).toBe("/es/docs/cli/#section-2");
		expect(localizedPath("pt", path)).toBe(path);
		expect(routeLocale("/docs/cli/")).toBe("en");
		expect(routeLocale("/es/docs/cli/")).toBe("es");
		expect(routeLocale("/pt/docs/cli/")).toBe("pt");
		expect(stripLocale("/en-us/docs/cli/")).toBe("/docs/cli/");
		expect(stripLocale("/es-es/docs/cli/")).toBe("/docs/cli/");
		expect(stripLocale("/pt-br/docs/cli/")).toBe("/docs/cli/");
		expect(localizedPath("es", "https://github.com/djalmajr/dukto")).toBe(
			"https://github.com/djalmajr/dukto",
		);
	});

	test("application navigation has English and Spanish translations", () => {
		expect(Object.keys(en).sort()).toEqual(Object.keys(es).sort());
		const navigation = new Set<string>();
		const file = ts.createSourceFile(
			"App.tsx",
			readFileSync(`${import.meta.dir}/App.tsx`, "utf8"),
			ts.ScriptTarget.Latest,
			true,
			ts.ScriptKind.TSX,
		);
		function visit(node: ts.Node) {
			if (
				ts.isCallExpression(node) &&
				node.expression.getText(file) === "t" &&
				node.arguments[0] &&
				ts.isStringLiteral(node.arguments[0])
			)
				navigation.add(node.arguments[0].text);
			ts.forEachChild(node, visit);
		}
		visit(file);

		for (const message of navigation) {
			expect((en as Record<string, string>)[message], `Missing English: ${message}`).toBeTruthy();
			expect((es as Record<string, string>)[message], `Missing Spanish: ${message}`).toBeTruthy();
		}
	});

	test("every guide string has Spanish and Portuguese content", () => {
		const documentContent = new Set<string>();
		for (const guide of guides) {
			for (const text of [guide.navTitle, guide.navGroup, guide.title, guide.group, guide.summary])
				documentContent.add(text);
			for (const section of guide.sections) {
				documentContent.add(section.title);
				if (section.code) documentContent.add(section.code);
				if (section.text) documentContent.add(section.text);
				if (section.link) documentContent.add(section.link.label);
				for (const item of section.items || []) documentContent.add(item);
			}
		}

		for (const message of documentContent) {
			expect(Object.hasOwn(guideEs, message), `Missing Spanish guide translation: ${message}`).toBe(
				true,
			);
			expect(
				Object.hasOwn(guidePt, message),
				`Missing Portuguese guide translation: ${message}`,
			).toBe(true);
		}
	});

	test("privacy guide discloses operated internet services in every language", () => {
		const privacy = guides.find((guide) => guide.slug === "privacy");
		if (!privacy) throw new Error("Privacy guide is missing");
		const english = [privacy.summary, ...privacy.sections.map((section) => section.text ?? "")];
		const portuguese = english.map((message) => translate("pt", message));
		const spanish = english.map((message) => translate("es", message));

		expect(english.join(" ")).toContain("relay services");
		expect(english.join(" ")).toContain("source IP addresses");
		expect(portuguese.join(" ")).toContain("endereços IP de origem");
		expect(spanish.join(" ")).toContain("direcciones IP de origen");
		for (const content of [english, portuguese, spanish]) {
			expect(content.join(" ")).not.toContain(
				"Dukto does not route or retain this information through a developer-operated server",
			);
		}
	});

	test("guide metadata follows the selected language", () => {
		expect(pageMetadata("/docs/getting-started/")).toEqual(
			expect.objectContaining({
				description: "Two devices. One direct connection. Your files where they belong.",
				locale: "en",
				title: "Getting started — Dukto Docs",
			}),
		);
		expect(pageMetadata("/es/docs/getting-started/")).toEqual(
			expect.objectContaining({
				description: "Dos dispositivos. Una conexión directa. Tus archivos donde deben estar.",
				locale: "es",
				title: "Primeros pasos — Dukto Docs",
			}),
		);
		expect(pageMetadata("/pt/docs/getting-started/")).toEqual(
			expect.objectContaining({
				description: "Dois dispositivos. Uma conexão direta. Seus arquivos onde devem estar.",
				locale: "pt",
				title: "Primeiros passos — Dukto Docs",
			}),
		);
		expect(pageMetadata("/es/404.html").title).toBe("Página no encontrada — Dukto");
		expect(translate("pt", "Getting started")).toBe("Primeiros passos");
	});
});
