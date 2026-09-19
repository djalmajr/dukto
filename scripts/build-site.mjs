import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { build } from "vite";
await build({ configFile: "site/vite.config.ts" });
await build({
	configFile: "site/vite.config.ts",
	build: {
		ssr: resolve("site/src/entry-server.tsx"),
		outDir: resolve(".cache/site-ssr"),
		emptyOutDir: true,
	},
});
const { renderPage, guides, generateHydrationScript, locales, localizedPath, pageMetadata } =
	await import(pathToFileURL(resolve(".cache/site-ssr/entry-server.js")));
const shell = await readFile("site/dist/index.html", "utf8");
const baseRoutes = [
	{ path: "/", title: "Dukto — Your files, right there." },
	{ path: "/downloads/", title: "Downloads — Dukto" },
	{ path: "/docs/", title: "Documentation — Dukto" },
	...guides.map((g) => ({
		path: `/docs/${g.slug}/`,
		title: `${g.title} — Dukto Docs`,
		description: g.summary,
	})),
	{ path: "/404.html", title: "Page not found — Dukto" },
];
const routes = locales.flatMap((locale) =>
	baseRoutes.map((route) => ({
		...route,
		path: localizedPath(locale, route.path),
		...pageMetadata(localizedPath(locale, route.path)),
	})),
);
const escapeHtml = (s) =>
	s.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;");
for (const route of routes) {
	const file = route.path.endsWith("404.html")
		? `site/dist${route.path}`
		: `site/dist${route.path}index.html`;
	const html = shell
		.replace(
			"</head>",
			`${generateHydrationScript()}${locales.map((locale) => `<link rel="alternate" hreflang="${locale}" href="https://dukto.app${localizedPath(locale, route.path)}" />`).join("")}<link rel="alternate" hreflang="x-default" href="https://dukto.app${localizedPath("en", route.path)}" /></head>`,
		)
		.replace(/<html lang="[^"]+"/, `<html lang="${route.locale}"`)
		.replace('<div id="root"></div>', `<div id="root">${renderPage(route.path)}</div>`)
		.replace(/<title>.*?<\/title>/, `<title>${escapeHtml(route.title)}</title>`)
		.replace('href="https://dukto.app/"', `href="https://dukto.app${route.path}"`)
		.replace(
			/(<meta name="description" content=")[^"]*/,
			`$1${escapeHtml(route.description || "Files and folders between macOS, Windows, Linux, Android, and iOS, directly between your devices.")}`,
		);
	await mkdir(resolve(file, ".."), { recursive: true });
	await writeFile(file, html);
}
await writeFile(
	"site/dist/sitemap.xml",
	`<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${routes
		.filter((r) => !r.path.endsWith("404.html"))
		.map((r) => `<url><loc>https://dukto.app${r.path}</loc></url>`)
		.join("")}</urlset>`,
);
console.log(`Prerendered ${routes.length} pages.`);
