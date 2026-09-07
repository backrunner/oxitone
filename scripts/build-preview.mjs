import { execFileSync } from "node:child_process";
import { copyFileSync, mkdirSync, renameSync, rmSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

const root = fileURLToPath(new URL("../", import.meta.url));
const profile = process.argv.includes("--debug") ? "debug" : "release";
execFileSync("cargo", ["build", "--locked", "-p", "oxitone-preview", ...(profile === "release" ? ["--release"] : [])], { cwd: root, stdio: "inherit" });
const contents = join(root, "target", profile, "Oxitone Preview.app", "Contents");
mkdirSync(join(contents, "MacOS"), { recursive: true });
const executable = join(contents, "MacOS", "oxitone-preview");
const temporary = `${executable}.${process.pid}.tmp`;
try {
  // A fresh inode avoids macOS reusing the prior executable's cached code signature.
  copyFileSync(join(root, "target", profile, "oxitone-preview"), temporary);
  renameSync(temporary, executable);
} finally {
  rmSync(temporary, { force: true });
}
copyFileSync(join(root, "apps/preview/Info.plist"), join(contents, "Info.plist"));
console.log(`Built ${join(contents, "..")} (unsigned development bundle)`);
