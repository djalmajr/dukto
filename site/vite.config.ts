import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import tailwindcss from "@tailwindcss/vite";
import Icons from "unplugin-icons/vite";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

const root = fileURLToPath(new URL(".", import.meta.url));
export default defineConfig({
	root,
	plugins: [solid({ ssr: true }), tailwindcss(), Icons({ compiler: "solid" })],
	resolve: { alias: { "~": resolve(root, "../src") } },
	server: { host: "127.0.0.1", port: 4178, strictPort: true },
	build: { outDir: "dist", emptyOutDir: true },
});
