import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const root = fileURLToPath(new URL("../", import.meta.url));
const profile = process.argv.includes("--debug") ? "debug" : "release";
execFileSync("cargo", ["build", "--locked", "-p", "oxitone-preview", ...(profile === "release" ? ["--release"] : [])], { cwd: root, stdio: "inherit" });
const contents = join(root, "target", profile, "Oxitone Preview.app", "Contents");
mkdirSync(join(contents, "MacOS"), { recursive: true });
copyFileSync(join(root, "target", profile, "oxitone-preview"), join(contents, "MacOS", "oxitone-preview"));
copyFileSync(join(root, "apps/preview/Info.plist"), join(contents, "Info.plist"));
console.log(`Built ${join(contents, "..")} (unsigned development bundle)`);
