import { readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import type { Plugin } from "vite";

const VIRTUAL_MODULE_ID = "virtual:i18n";
const RESOLVED_ID = `\0${VIRTUAL_MODULE_ID}`;

interface TranslationEntry {
	lang: string;
	namespace: string;
	path: string;
}

interface Options {
	dirs: string[];
}

function findTranslationFiles(dir: string, baseDir: string): TranslationEntry[] {
	const entries: TranslationEntry[] = [];

	function scan(currentDir: string) {
		const items = readdirSync(currentDir);

		for (const item of items) {
			const fullPath = join(currentDir, item);
			const stat = statSync(fullPath);

			if (stat.isDirectory()) {
				if (item === "node_modules" || item === "dist") continue;
				scan(fullPath);
			} else if (item.endsWith(".json") && currentDir.endsWith("/locales")) {
				const relativePath = relative(baseDir, currentDir);
				const parts = relativePath.split("/").filter((p) => p !== "locales");

				if (parts[0] === "routes") parts.shift();

				const namespace = parts.length > 0 ? parts.join(".") : "common";
				const lang = item.replace(".json", "");

				entries.push({ lang, namespace, path: fullPath });
			}
		}
	}

	scan(dir);
	return entries;
}

function generateModule(entries: TranslationEntry[]): string {
	const namespaces = new Map<string, Map<string, string>>();

	for (const entry of entries) {
		if (!namespaces.has(entry.namespace)) {
			namespaces.set(entry.namespace, new Map());
		}
		namespaces.get(entry.namespace)!.set(entry.lang, entry.path);
	}

	const imports: string[] = [];
	const mapEntries: string[] = [];
	let i = 0;

	for (const [namespace, langs] of namespaces) {
		const langEntries: string[] = [];

		for (const [lang, path] of langs) {
			const name = `t${i++}`;
			imports.push(`const ${name} = () => import("${path}");`);
			langEntries.push(`    "${lang}": ${name}`);
		}

		mapEntries.push(`  "${namespace}": {\n${langEntries.join(",\n")}\n  }`);
	}

	return `${imports.join("\n")}

export const translations = {
${mapEntries.join(",\n")}
};
`;
}

export function i18nextColocated(options: Options): Plugin {
	return {
		name: "vite-plugin-i18next-colocated",

		resolveId(id) {
			if (id === VIRTUAL_MODULE_ID) return RESOLVED_ID;
		},

		load(id) {
			if (id !== RESOLVED_ID) return;

			const allEntries: TranslationEntry[] = [];
			for (const dir of options.dirs) {
				allEntries.push(...findTranslationFiles(dir, dir));
			}

			console.log(`[i18next] Found ${allEntries.length} translation files`);
			return generateModule(allEntries);
		},
	};
}
