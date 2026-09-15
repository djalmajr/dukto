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
	const locale = file.replaceAll("\\", "/").includes("/en-us/")
		? "en-us"
		: file.replaceAll("\\", "/").includes("/es-es/")
			? "es-es"
			: "pt-br";
	assert.ok(html.includes(`<html lang="${locale}"`), file);
	assert.match(html, /hreflang="en-us"/, file);
	assert.match(html, /hreflang="es-es"/, file);
	assert.match(html, /data-hk=/, `${file}: missing prerendered content`);
	assert.match(html, /\$HY/, `${file}: missing hydration script`);
	for (const [, href] of html.matchAll(/(?:href|src)="(\/[^"#?]*)"/g)) {
		await access(join(root, href.endsWith("/") ? `${href}index.html` : href));
	}
}
console.log(`PASS ${files.length} prerendered pages: headings, hydration and internal file links`);
