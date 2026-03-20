import i18n from "i18next";
import LanguageDetector from "i18next-browser-languagedetector";
import { createSignal } from "solid-js";
import en from "../routes/locales/en.json";
import es from "../routes/locales/es.json";
import pt from "../routes/locales/pt.json";

i18n.use(LanguageDetector).init({
	defaultNS: "common",
	detection: {
		caches: ["localStorage"],
		convertDetectedLanguage: (lng: string) => lng.split("-")[0] ?? lng,
		lookupLocalStorage: "dukto:language",
		order: ["localStorage", "navigator"],
	},
	fallbackLng: "pt",
	interpolation: { escapeValue: false },
	resources: {
		en: { common: en },
		es: { common: es },
		pt: { common: pt },
	},
	supportedLngs: ["en", "es", "pt"],
});

const [language, setLanguage] = createSignal(i18n.language);

export function changeLanguage(lang: string) {
	i18n.changeLanguage(lang);
	setLanguage(lang);
}

export function t(key: string, options?: Record<string, unknown>): string {
	language();
	return i18n.t(key, options);
}

export { language };
export default i18n;
