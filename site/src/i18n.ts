import { createContext, useContext } from "solid-js";
import en from "./locales/en-us.json";
import es from "./locales/es-es.json";
import guideEs from "./locales/guides.es.json";
import guidePt from "./locales/guides.pt.json";

export const locales = ["en", "es", "pt"] as const;
export type Locale = (typeof locales)[number];
export const languageNames: Record<Locale, string> = {
	en: "English",
	es: "Español",
	pt: "Português",
};
const dictionaries: Record<Locale, Record<string, string>> = {
	en,
	es: { ...es, ...guideEs },
	pt: guidePt,
};
export function translate(locale: Locale, source: string) {
	return dictionaries[locale]?.[source] ?? source;
}
export function routeLocale(path: string): Locale {
	return path.startsWith("/es/") || path === "/es"
		? "es"
		: path.startsWith("/pt/") || path === "/pt"
			? "pt"
			: "en";
}
export function stripLocale(path: string) {
	return path.replace(/^\/(en|es|pt|en-us|es-es|pt-br)(?=\/|$)/, "") || "/";
}
export function localizedPath(locale: Locale, path: string) {
	if (!path.startsWith("/") || path.startsWith("//")) return path;
	return (locale === "en" ? "" : `/${locale}`) + stripLocale(path);
}
export function localeValue(locale: Locale) {
	return {
		locale,
		t: (source: string) => translate(locale, source),
		localPath: (path: string) => localizedPath(locale, path),
	};
}
export const LocaleContext = createContext(localeValue("en"));
export const useLocale = () => useContext(LocaleContext);
