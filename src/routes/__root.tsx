import { Outlet, createRootRoute } from "@tanstack/solid-router";
import { createEffect } from "solid-js";
import { resolvedTheme } from "~/stores/settings";

function RootLayout() {
	createEffect(() => {
		const dark = resolvedTheme() === "dark";
		document.documentElement.classList.toggle("dark", dark);
		document.documentElement.setAttribute("data-kb-theme", dark ? "dark" : "light");
	});

	return <Outlet />;
}

export const Route = createRootRoute({
	component: RootLayout,
});
