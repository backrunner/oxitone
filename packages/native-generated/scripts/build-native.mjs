import { execFileSync } from "node:child_process";
import { copyFileSync, readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const packageDir = join(here, "..");
const repoRoot = join(packageDir, "..", "..");
const crateDir = join(repoRoot, "crates", "napi");
const napiBin = join(
  repoRoot,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "napi.cmd" : "napi",
);

execFileSync(napiBin, ["build", "--platform", "--release"], {
  cwd: crateDir,
  stdio: "inherit",
});

for (const entry of readdirSync(crateDir)) {
  if (entry.endsWith(".node") || entry === "index.js" || entry === "index.d.ts") {
    copyFileSync(join(crateDir, entry), join(packageDir, entry));
  }
}
