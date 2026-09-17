import { readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const root = process.cwd();
const androidRoot = path.join(root, "src-tauri", "gen", "android");
const appRoot = path.join(androidRoot, "app");
const manifestPath = path.join(appRoot, "src", "main", "AndroidManifest.xml");
const gradlePath = path.join(appRoot, "build.gradle.kts");

async function findFile(directory, filename) {
	const entries = await readdir(directory, { withFileTypes: true });
	for (const entry of entries) {
		const candidate = path.join(directory, entry.name);
		if (entry.isDirectory()) {
			const nested = await findFile(candidate, filename);
			if (nested) return nested;
		} else if (entry.name === filename) {
			return candidate;
		}
	}
	return null;
}

function requireMatch(source, pattern, message) {
	if (!pattern.test(source)) {
		throw new Error(message);
	}
}

async function configureManifest() {
	let manifest = await readFile(manifestPath, "utf8");
	const permissions = [
		"android.permission.CHANGE_WIFI_MULTICAST_STATE",
		"android.permission.ACCESS_NETWORK_STATE",
		"android.permission.ACCESS_WIFI_STATE",
	];

	for (const permission of permissions) {
		if (!manifest.includes(permission)) {
			manifest = manifest.replace(
				/(\s*<uses-permission android:name="android\.permission\.INTERNET" \/>)/,
				`$1\n    <uses-permission android:name="${permission}" />`,
			);
		}
	}

	manifest = manifest
		.replace(
			/\s*<!-- AndroidTV support -->\s*<uses-feature[^>]*android\.software\.leanback[^>]*\/>/g,
			"",
		)
		.replace(
			/\s*<!-- AndroidTV support -->\s*<category android:name="android\.intent\.category\.LEANBACK_LAUNCHER" \/>/g,
			"",
		);

	requireMatch(
		manifest,
		/android\.permission\.INTERNET/,
		"Android manifest lost INTERNET permission",
	);
	for (const permission of permissions) {
		requireMatch(manifest, new RegExp(permission.replaceAll(".", "\\.")), `Missing ${permission}`);
	}
	if (manifest.includes("LEANBACK_LAUNCHER") || manifest.includes("android.software.leanback")) {
		throw new Error("Android TV declarations were not removed");
	}

	await writeFile(manifestPath, manifest, "utf8");
}

async function configureMainActivity() {
	const javaRoot = path.join(appRoot, "src", "main", "java");
	const activityPath = await findFile(javaRoot, "MainActivity.kt");
	if (!activityPath) throw new Error(`MainActivity.kt was not found under ${javaRoot}`);

	const current = await readFile(activityPath, "utf8");
	const packageMatch = current.match(/^package\s+([\w.]+)/m);
	if (!packageMatch) throw new Error("Could not read the Android package from MainActivity.kt");

	const source = `package ${packageMatch[1]}

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private var multicastLock: WifiManager.MulticastLock? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    val wifiManager = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
    multicastLock = wifiManager.createMulticastLock("dukto-mdns").apply {
      setReferenceCounted(false)
      acquire()
    }
  }

  override fun onDestroy() {
    multicastLock?.let { lock ->
      if (lock.isHeld) lock.release()
    }
    multicastLock = null
    super.onDestroy()
  }
}
`;

	await writeFile(activityPath, source, "utf8");
}

async function verifySdk() {
	const gradle = await readFile(gradlePath, "utf8");
	requireMatch(gradle, /compileSdk\s*=\s*36\b/, "Android compileSdk must be 36");
	requireMatch(gradle, /targetSdk\s*=\s*36\b/, "Android targetSdk must be 36");
}

await configureManifest();
await configureMainActivity();
await verifySdk();

console.log("Android project configured for Dukto.");
