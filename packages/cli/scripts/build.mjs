import { spawnSync } from "node:child_process";
import { rm } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

// tsc alone leaves deleted source modules in dist and would publish obsolete implementations.
await rm(new URL("../dist/", import.meta.url), { recursive: true, force: true });
const compiler = createRequire(import.meta.url).resolve("typescript/bin/tsc");
const result = spawnSync(process.execPath, [compiler, "-p", fileURLToPath(new URL("../tsconfig.build.json", import.meta.url))], { stdio: "inherit" });
if (result.error) throw result.error;
process.exitCode = result.status ?? 1;
