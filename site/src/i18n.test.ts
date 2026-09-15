import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import ts from "typescript";
import { guides } from "./guides";
import { localizedPath, routeLocale, stripLocale, translate } from "./i18n";
import en from "./locales/en-us.json";
import es from "./locales/es-es.json";
import { pageMetadata } from "./metadata";

describe("website localization", () => {
	test("language switching preserves the guide and anchor without duplicate prefixes", () => {
		const path = "/en-us/docs/cli/#section-2";
		expect(localizedPath("es-es", path)).toBe("/es-es/docs/cli/#section-2");
		expect(localizedPath("pt-br", path)).toBe("/docs/cli/#section-2");
		expect(localizedPath("en-us", path)).toBe(path);
		expect(stripLocale("/es-es/")).toBe("/");
		expect(routeLocale("/en-us/")).toBe("en-us");
		expect(routeLocale("/docs/cli/")).toBe("pt-br");
		expect(localizedPath("es-es", "https://github.com/djalmajr/dukto")).toBe(
			"https://github.com/djalmajr/dukto",
		);
	});
	test("navigation is localized while documentation content remains English", () => {
		expect(Object.keys(en).sort()).toEqual(Object.keys(es).sort());
		const navigation = new Set<string>();
		const documentContent = new Set<string>();
		for (const guide of guides) {
			navigation.add(guide.navTitle);
			navigation.add(guide.navGroup);
			for (const text of [guide.title, guide.group, guide.summary]) documentContent.add(text);
			for (const section of guide.sections) {
				documentContent.add(section.title);
				if (section.text) documentContent.add(section.text);
				for (const item of section.items || []) documentContent.add(item);
				if (section.code) documentContent.add(section.code);
			}
		}
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
		for (const message of documentContent) {
			expect(
				(en as Record<string, string>)[message],
				`Documentation should not be translated: ${message}`,
			).toBeUndefined();
			expect(
				(es as Record<string, string>)[message],
				`Documentation should not be translated: ${message}`,
			).toBeUndefined();
		}
	});
	test("guide metadata and document language stay en-US on every route", () => {
		for (const path of [
			"/docs/primeiros-passos/",
			"/en-us/docs/primeiros-passos/",
			"/es-es/docs/primeiros-passos/",
		]) {
			const metadata = pageMetadata(path);
			expect(metadata.locale).toBe("en-US");
			expect(metadata.title).toBe("Getting started — Dukto Docs");
			expect(metadata.description).toBe(
				"Two computers. One network. Your files where they belong.",
			);
		}
		expect(pageMetadata("/es-es/404.html").title).toBe("Página no encontrada — Dukto");
		expect(translate("pt-br", "Primeiros passos")).toBe("Primeiros passos");
	});
});
