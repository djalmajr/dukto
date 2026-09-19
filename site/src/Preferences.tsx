import { For, Show, createSignal, onCleanup, onMount } from "solid-js";
import IconLanguages from "~icons/lucide/languages";
import IconMoon from "~icons/lucide/moon";
import IconSun from "~icons/lucide/sun";
import { Button } from "../../src/components/ui/button";
import { languageNames, locales, localizedPath, useLocale } from "./i18n";
import { isOutsideLanguagePicker } from "./language-picker";

export function Preferences() {
	const { t, locale } = useLocale();
	const [dark, setDark] = createSignal(false);
	const [open, setOpen] = createSignal(false);
	const [path, setPath] = createSignal("/");
	let languageButton: HTMLButtonElement | undefined;
	let languagePicker: HTMLDivElement | undefined;
	onMount(() => {
		setDark(document.documentElement.dataset.theme === "dark");
		setPath(window.location.pathname + window.location.hash);
		const closeWhenOutside = (event: PointerEvent) => {
			if (open() && isOutsideLanguagePicker(languagePicker, event.target)) setOpen(false);
		};
		const closeOnEscape = (event: KeyboardEvent) => {
			if (!open() || event.key !== "Escape") return;
			setOpen(false);
			languageButton?.focus();
		};
		document.addEventListener("pointerdown", closeWhenOutside);
		document.addEventListener("keydown", closeOnEscape);
		onCleanup(() => {
			document.removeEventListener("pointerdown", closeWhenOutside);
			document.removeEventListener("keydown", closeOnEscape);
		});
	});
	function toggleTheme() {
		const next = !dark();
		setDark(next);
		document.documentElement.dataset.theme = next ? "dark" : "light";
		document.documentElement.dataset.kbTheme = next ? "dark" : "light";
		try {
			localStorage.setItem("dukto-site-theme", next ? "dark" : "light");
		} catch {
			/* Storage may be disabled. */
		}
	}
	return (
		<div class="preferences">
			<Button
				variant="ghost"
				size="icon"
				class="preference-button"
				onClick={toggleTheme}
				aria-label={t(dark() ? "Tema claro" : "Tema escuro")}
				title={t(dark() ? "Tema claro" : "Tema escuro")}
			>
				<Show when={dark()} fallback={<IconMoon aria-hidden="true" />}>
					<IconSun aria-hidden="true" />
				</Show>
			</Button>
			<div class="language-picker" ref={languagePicker}>
				<Button
					ref={languageButton}
					variant="ghost"
					class="preference-button"
					size="icon"
					title={t("Idioma")}
					aria-label={t("Idioma")}
					aria-expanded={open()}
					aria-controls="language-options"
					onClick={() => {
						setPath(window.location.pathname + window.location.hash);
						setOpen(!open());
					}}
				>
					<IconLanguages aria-hidden="true" />
				</Button>
				<Show when={open()}>
					<nav id="language-options" class="language-options" aria-label={t("Idioma")}>
						<For each={locales}>
							{(lang) => (
								<a
									lang={lang}
									href={localizedPath(lang, path())}
									aria-current={locale === lang ? "true" : undefined}
								>
									{languageNames[lang]}
								</a>
							)}
						</For>
					</nav>
				</Show>
			</div>
		</div>
	);
}
