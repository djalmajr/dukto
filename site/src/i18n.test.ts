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
		const path = "/en-us/docs/cli/#secao-2";
		expect(localizedPath("es-es", path)).toBe("/es-es/docs/cli/#secao-2");
		expect(localizedPath("pt-br", path)).toBe("/docs/cli/#secao-2");
		expect(localizedPath("en-us", path)).toBe(path);
		expect(stripLocale("/es-es/")).toBe("/");
		expect(routeLocale("/en-us/")).toBe("en-us");
		expect(routeLocale("/docs/cli/")).toBe("pt-br");
		expect(localizedPath("es-es", "https://github.com/djalmajr/dukto")).toBe(
			"https://github.com/djalmajr/dukto",
		);
	});
	test("both translations cover every guide, item, and literal UI message", () => {
		expect(Object.keys(en).sort()).toEqual(Object.keys(es).sort());
		const messages = new Set<string>();
		for (const guide of guides) {
			for (const text of [guide.title, guide.summary, guide.group]) messages.add(text);
			for (const section of guide.sections) {
				messages.add(section.title);
				if (section.text) messages.add(section.text);
				for (const item of section.items || []) messages.add(item);
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
				messages.add(node.arguments[0].text);
			ts.forEachChild(node, visit);
		}
		visit(file);
		for (const message of messages) {
			expect((en as Record<string, string>)[message], `Missing English: ${message}`).toBeTruthy();
			expect((es as Record<string, string>)[message], `Missing Spanish: ${message}`).toBeTruthy();
		}
	});
	test("page metadata follows the locale including not found pages", () => {
		expect(pageMetadata("/en-us/docs/primeiros-passos/").title).toBe(
			"Getting started — Dukto Docs",
		);
		expect(pageMetadata("/es-es/404.html").title).toBe("Página no encontrada — Dukto");
		expect(translate("pt-br", "Primeiros passos")).toBe("Primeiros passos");
	});
});
