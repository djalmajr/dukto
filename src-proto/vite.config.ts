import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { TanStackRouterVite } from "@tanstack/router-plugin/vite";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import { i18nextColocated } from "../src/plugins/i18next";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
	plugins: [
		TanStackRouterVite({
			autoCodeSplitting: true,
			generatedRouteTree: "./src/routeTree.gen.ts",
			routesDirectory: "./src/routes",
			target: "solid",
		}),
		solid(),
		tailwindcss(),
		i18nextColocated({ dirs: [resolve(__dirname, "src")] }),
	],
	server: { port: 3333 },
	resolve: {
		alias: [
			{ find: "~/lib/i18n", replacement: resolve(__dirname, "src/lib/i18n.ts") },
			{ find: "~", replacement: resolve(__dirname, "../src") },
			{ find: "@app", replacement: resolve(__dirname, "../src") },
		],
	},
});
