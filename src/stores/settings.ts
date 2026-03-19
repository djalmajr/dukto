import { invoke } from "@tauri-apps/api/core";
import { createResource, createSignal } from "solid-js";

export interface AppSettings {
	destination_dir: string;
}

export type ThemeMode = "light" | "dark" | "system";

const THEME_STORAGE_KEY = "dukto-theme";

const [theme, setThemeSignal] = createSignal<ThemeMode>(
	(localStorage.getItem(THEME_STORAGE_KEY) as ThemeMode) || "system",
);

async function fetchSettings(): Promise<AppSettings> {
	return invoke<AppSettings>("get_settings");
}

const [settings, { refetch: refetchSettings }] = createResource<AppSettings>(fetchSettings);

function resolvedTheme(): "light" | "dark" {
	const selectedTheme = theme();
	if (selectedTheme === "system") {
		return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
	}
	return selectedTheme;
}

function setTheme(next: ThemeMode) {
	setThemeSignal(next);
	localStorage.setItem(THEME_STORAGE_KEY, next);
}

async function setDestinationDir(path: string) {
	await invoke("set_destination_dir", { path });
	refetchSettings();
}

export { settings, theme, resolvedTheme, setTheme, setDestinationDir };
