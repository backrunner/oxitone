import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { pluginManifestSchema, type RegisterPluginOptions } from "@oxitone/protocol";

const root = fileURLToPath(new URL("../../../", import.meta.url));

/** Explicit, local-only development libraries; no downloads or directory scanning. */
export function buildPlugins(output: string): RegisterPluginOptions[] {
  mkdirSync(output, { recursive: true });
  execFileSync("cargo", ["build", "--locked", "--release", "-p", "oxitone-example-drums"],
    { cwd: root, stdio: "inherit" });
  const suffix = process.platform === "darwin" ? ".dylib" : ".so";
  const effect = join(output, `reference-gain${suffix}`);
  execFileSync("cc", ["-std=c11", "-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror",
    "-I", join(root, "include"), join(root, "crates/render/tests/fixtures/gain.c"), "-o", effect]);
  return [
    [join(root, `target/release/liboxitone_example_drums${suffix}`), join(root, "crates/example-drums/manifest.json")],
    [effect, join(root, "crates/render/tests/fixtures/gain.json")],
  ].map(([libraryPath, manifestPath]) => ({
    libraryPath: libraryPath!,
    expectedHash: createHash("sha256").update(readFileSync(libraryPath!)).digest("hex"),
    manifest: pluginManifestSchema.parse(JSON.parse(readFileSync(manifestPath!, "utf8"))),
  }));
}
