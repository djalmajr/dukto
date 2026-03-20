import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { TanStackRouterVite } from "@tanstack/router-plugin/vite";
import Icons from "unplugin-icons/vite";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";
import { i18nextColocated } from "./src/plugins/i18next";

const __dirname = fileURLToPath(new URL(".", import.meta.url));
const host = process.env.TAURI_DEV_HOST;

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
		Icons({ compiler: "solid" }),
		i18nextColocated({ dirs: [resolve(__dirname, "src")] }),
	],
	clearScreen: false,
	resolve: {
		alias: {
			"~": resolve(__dirname, "./src"),
		},
	},
	server: {
		host: host || false,
		port: 1420,
		strictPort: true,
		hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
		watch: { ignored: ["**/src-tauri/**"] },
	},
});
