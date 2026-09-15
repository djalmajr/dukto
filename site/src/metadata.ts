import { guides } from "./guides";
import { routeLocale, stripLocale, translate } from "./i18n";
export function pageMetadata(path: string) {
	const locale = routeLocale(path);
	const clean = stripLocale(path).replace(/\/+$/, "") || "/";
	const guide = guides.find((g) => clean === `/docs/${g.slug}`);
	const t = (text: string) => translate(locale, text);
	return {
		locale,
		title: guide
			? `${t(guide.title)} — Dukto Docs`
			: clean === "/downloads"
				? `${t("Downloads")} — Dukto`
				: clean === "/docs"
					? `${t("Documentação")} — Dukto`
					: clean === "/"
						? t("Dukto — Seus arquivos, logo ali.")
						: t("Página não encontrada — Dukto"),
		description: t(
			guide?.summary || "Arquivos e pastas entre Mac, Windows e Linux, direto pela rede local.",
		),
	};
}
