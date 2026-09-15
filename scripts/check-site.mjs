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
	const routeLocale = normalizedFile.includes("/en-us/")
		? "en-us"
		: normalizedFile.includes("/es-es/")
			? "es-es"
			: "pt-br";
	const isGuidePage = /\/docs\/[^/]+\/index\.html$/.test(normalizedFile);
	const expectedHtmlLocale = isGuidePage ? "en-us" : routeLocale;
	const htmlLocale = html.match(/<html lang="([^"]+)"/)?.[1]?.toLowerCase();
	assert.equal(htmlLocale, expectedHtmlLocale, `${file}: unexpected document language`);
	if (isGuidePage) {
		const sidebar = html.match(/<aside class="docs-sidebar">([\s\S]*?)<\/aside>/)?.[1];
		assert.ok(sidebar, `${file}: missing documentation sidebar`);
		const localizedNavigation = {
			"pt-br": ["USANDO O DUKTO", "Arquivos e pastas"],
			"en-us": ["USING DUKTO", "Files and folders"],
			"es-es": ["USAR DUKTO", "Archivos y carpetas"],
		}[routeLocale];
		for (const label of localizedNavigation) {
			assert.ok(sidebar.includes(label), `${file}: missing localized navigation label: ${label}`);
		}
	}
	assert.match(html, /hreflang="en-us"/, file);
	assert.match(html, /hreflang="es-es"/, file);
	assert.match(html, /data-hk=/, `${file}: missing prerendered content`);
	assert.match(html, /\$HY/, `${file}: missing hydration script`);
	for (const [, href] of html.matchAll(/(?:href|src)="(\/[^"#?]*)"/g)) {
		await access(join(root, href.endsWith("/") ? `${href}index.html` : href));
	}
}
console.log(`PASS ${files.length} prerendered pages: headings, hydration and internal file links`);
