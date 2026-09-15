import { createContext, useContext } from "solid-js";
import en from "./locales/en-us.json";
import es from "./locales/es-es.json";

export const locales = ["pt-br", "en-us", "es-es"] as const;
export type Locale = (typeof locales)[number];
export const languageNames: Record<Locale, string> = {
	"pt-br": "Português (Brasil)",
	"en-us": "English (US)",
	"es-es": "Español (España)",
};
const dictionaries: Record<string, Record<string, string>> = { "en-us": en, "es-es": es };
export function translate(locale: Locale, source: string) {
	return dictionaries[locale]?.[source] ?? source;
}
export function routeLocale(path: string): Locale {
	return path.startsWith("/en-us/") || path === "/en-us"
		? "en-us"
		: path.startsWith("/es-es/") || path === "/es-es"
			? "es-es"
			: "pt-br";
}
export function stripLocale(path: string) {
	return path.replace(/^\/(en-us|es-es)(?=\/|$)/, "") || "/";
}
export function localizedPath(locale: Locale, path: string) {
	if (!path.startsWith("/") || path.startsWith("//")) return path;
	return (locale === "pt-br" ? "" : `/${locale}`) + stripLocale(path);
}
export function localeValue(locale: Locale) {
	return {
		locale,
		t: (source: string) => translate(locale, source),
		localPath: (path: string) => localizedPath(locale, path),
	};
}
export const LocaleContext = createContext(localeValue("pt-br"));
export const useLocale = () => useContext(LocaleContext);
