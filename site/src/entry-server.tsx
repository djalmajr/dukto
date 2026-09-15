import { renderToString } from "solid-js/web";
export { generateHydrationScript } from "solid-js/web";
import { App } from "./App";
export { guides } from "./guides";
export function renderPage(path: string) {
	return renderToString(() => <App path={path} />);
}

export { locales, localizedPath } from "./i18n";
export { pageMetadata } from "./metadata";
