import assert from "node:assert/strict";
import { access, readFile, readdir } from "node:fs/promises";
import { join } from "node:path";
const root = "site/dist";
async function pages(dir) {
	const out = [];
	for (const e of await readdir(dir, { withFileTypes: true })) {
		const p = join(dir, e.name);
		if (e.isDirectory()) out.push(...(await pages(p)));
		else if (e.name.endsWith(".html")) out.push(p);
	}
	return out;
}
const files = await pages(root);
assert.equal(files.length, 33);
for (const file of files) {
	const html = await readFile(file, "utf8");
	assert.equal((html.match(/<h1[\s>]/g) || []).length, 1, file);
	const normalizedFile = file.replaceAll("\\", "/");
	const routeLocale = normalizedFile.includes("/es/")
		? "es"
		: normalizedFile.includes("/pt/")
			? "pt"
			: "en";
	const isGuidePage = /\/docs\/[^/]+\/index\.html$/.test(normalizedFile);
	const htmlLocale = html.match(/<html lang="([^"]+)"/)?.[1]?.toLowerCase();
	assert.equal(htmlLocale, routeLocale, `${file}: unexpected document language`);
	if (isGuidePage) {
		const sidebar = html.match(/<aside class="docs-sidebar">([\s\S]*?)<\/aside>/)?.[1];
		assert.ok(sidebar, `${file}: missing documentation sidebar`);
		const localizedNavigation = {
			en: ["USING DUKTO", "Files and folders"],
			es: ["USAR DUKTO", "Archivos y carpetas"],
			pt: ["USANDO O DUKTO", "Arquivos e pastas"],
		}[routeLocale];
		for (const label of localizedNavigation) {
			assert.ok(sidebar.includes(label), `${file}: missing localized navigation label: ${label}`);
		}
		if (normalizedFile.endsWith("/docs/getting-started/index.html")) {
			// Mutation captured: rendering raw guide fields leaves translated article bodies in English.
			const localizedArticle = {
				en: { heading: "Getting started", section: "Open Dukto on both devices" },
				es: { heading: "Primeros pasos", section: "Abre Dukto en ambos dispositivos" },
				pt: { heading: "Primeiros passos", section: "Abra o Dukto nos dois dispositivos" },
			}[routeLocale];
			assert.ok(
				html.includes(`<h1>${localizedArticle.heading}</h1>`),
				`${file}: missing localized article heading: ${localizedArticle.heading}`,
			);
			assert.ok(
				html.includes(localizedArticle.section),
				`${file}: missing localized article text: ${localizedArticle.section}`,
			);
		}
	}
	assert.match(html, /hreflang="en"/, file);
	assert.match(html, /hreflang="es"/, file);
	assert.match(html, /hreflang="pt"/, file);
	assert.match(html, /hreflang="x-default"/, file);
	assert.match(html, /data-hk=/, `${file}: missing prerendered content`);
	assert.match(html, /\$HY/, `${file}: missing hydration script`);
	for (const [, href] of html.matchAll(/(?:href|src)="(\/[^"#?]*)"/g)) {
		await access(join(root, href.endsWith("/") ? `${href}index.html` : href));
	}
}
console.log(`PASS ${files.length} prerendered pages: headings, hydration and internal file links`);
