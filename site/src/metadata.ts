import { guides } from "./guides";
import { routeLocale, stripLocale, translate } from "./i18n";
export function pageMetadata(path: string) {
	const routeLanguage = routeLocale(path);
	const clean = stripLocale(path).replace(/\/+$/, "") || "/";
	const guide = guides.find((g) => clean === `/docs/${g.slug}`);
	const t = (text: string) => translate(routeLanguage, text);
	return {
		locale: guide ? "en-US" : routeLanguage,
		title: guide
			? `${guide.title} — Dukto Docs`
			: clean === "/downloads"
				? `${t("Downloads")} — Dukto`
				: clean === "/docs"
					? `${t("Documentação")} — Dukto`
					: clean === "/"
						? t("Dukto — Seus arquivos, logo ali.")
						: t("Página não encontrada — Dukto"),
		description:
			guide?.summary ||
			t(
				"Arquivos e pastas entre macOS, Windows, Linux, Android e iOS, direto entre seus dispositivos.",
			),
	};
}
