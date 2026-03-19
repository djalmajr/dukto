import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
	plugins: [solid(), tailwindcss()],
	clearScreen: false,
	server: {
		host: host || false,
		port: 1420,
		strictPort: true,
		hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
		watch: { ignored: ["**/src-tauri/**"] },
	},
});
