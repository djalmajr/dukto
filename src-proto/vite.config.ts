import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

const __dirname = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
	plugins: [solid(), tailwindcss()],
	server: { port: 3333 },
	resolve: {
		alias: {
			"@app": resolve(__dirname, "../src"),
			"~": resolve(__dirname, "../src"),
		},
	},
});
