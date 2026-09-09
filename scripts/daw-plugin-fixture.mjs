import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

/** Actual local npm/C fixture; callers own the temporary directory. Never opens an audio output. */
export async function dawPluginFixture(root, entry) {
  const repo = fileURLToPath(new URL("../", import.meta.url));
  const pkg = join(root, "node_modules/rack-fixture"); await mkdir(pkg);
  await writeFile(join(pkg, "package.json"), '{"name":"rack-fixture","version":"1.0.0","type":"module","exports":"./index.js"}');
  const binary = join(pkg, "gain.dylib");
  await promisify(execFile)("cc", ["-std=c11", "-shared", "-fPIC", "-O2", "-I", join(repo, "include"), join(repo, "crates/render/tests/fixtures/gain.c"), "-o", binary]);
  const hash = value => createHash("sha256").update(value).digest("hex");
  const sha256 = hash(await readFile(binary));
  const manifest = JSON.parse(await readFile(join(repo, "crates/render/tests/fixtures/gain.json"), "utf8"));
  const factory = `export const registration = ${JSON.stringify({ libraryPath: binary, expectedHash: sha256, manifest })};
export function rack() { return [
{ pluginId: 'fixture.gain', pluginVersion: '1.0.0', parameters: { gain: .7 }, mix: .8 },
{ pluginId: 'fixture.gain', pluginVersion: '1.0.0', parameters: { gain: .4 }, bypass: true }
]; }`;
  await writeFile(join(pkg, "index.js"), factory);
  const source = (await readFile(entry, "utf8"))
    .replace("const phrase =", "import { registration, rack } from 'rack-fixture';\nimport { createAutomationNamespace as pluginMotion } from '@oxitone/core';\nconst phrase =")
    .replace("const keys =", "project.registerPlugin(registration, { allowPlugins: 'any' });\nconst keys =")
    .replace("name: 'Keys', instrument:", "name: 'Keys', effectChain: rack(), instrument:")
    .replace("export default project;", "project.addChannel({ name: 'Other rack', effectChain: rack() });\nkeys.effectInstances[0].param('gain').automate(pluginMotion().constant(.35));\nexport default project;");
  await writeFile(entry, source);
  return async () => {
    if (hash(await readFile(binary)) !== sha256 || await readFile(join(pkg, "index.js"), "utf8") !== factory) throw new Error("DAW mutated npm plugin files");
  };
}
