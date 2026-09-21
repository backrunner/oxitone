import { afterEach, expect, it } from "vitest";
import { chmod, mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { listVst3Classes, scanVst3Bundles } from "../src/discovery.js";

const roots: string[] = [];
afterEach(async () => {
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true });
});
async function directory() {
  const root = await mkdtemp(join(tmpdir(), "oxitone-vst3-discovery-"));
  roots.push(root);
  return root;
}
it.skipIf(process.platform !== "darwin")(
  "lists nested bundles without loading code or following symlinks",
  async () => {
    const root = await directory();
    const bundles = [join(root, "A.vst3"), join(root, "Vendor", "B.vst3")];
    for (const path of bundles) await mkdir(path, { recursive: true });
    await mkdir(join(bundles[0]!, "Inner.vst3"));
    await mkdir(join(root, ".hidden", "C.vst3"), { recursive: true });
    await symlink(bundles[0]!, join(root, "Link.vst3"), "dir");
    await writeFile(join(bundles[0]!, "crash.mjs"), "throw new Error('scan must not execute plugin code');");
    expect(await scanVst3Bundles({ directories: [root, root] })).toEqual(bundles);
    await expect(scanVst3Bundles({ directories: [join(root, "missing")] })).rejects.toMatchObject({
      code: "AssetUnavailable",
    });
    await expect(scanVst3Bundles({ directories: ["relative"] })).rejects.toMatchObject({ code: "PluginConfigInvalid" });
    const controller = new AbortController();
    controller.abort();
    await expect(scanVst3Bundles({ directories: [root], signal: controller.signal })).rejects.toMatchObject({
      name: "AbortError",
    });
  },
);
it.skipIf(process.platform !== "darwin")(
  "validates all audio class identities and rejects a mismatched bundle hash",
  async () => {
    const root = await directory(),
      hostPath = join(root, "helper.mjs"),
      bundlePath = join(root, "Fixture.vst3");
    await writeFile(
      hostPath,
      `#!${process.execPath}
import { readFileSync, writeSync } from 'node:fs';
const r = JSON.parse(readFileSync(0, 'utf8'));
if (r.operation !== 'listClasses' || 'classId' in r.source) throw new Error('wrong request shape');
writeSync(3, JSON.stringify({ protocolVersion: 1, bundlePath: r.source.bundlePath, sha256: 'a'.repeat(64), vendor: 'Fixture', classes: [
 { classId: '1'.repeat(32), name: 'Instrument', category: 'Audio Module Class', version: '1' },
 { classId: '2'.repeat(32), name: 'Effect', category: 'Audio Module Class', version: '1' }
]}));
`,
    );
    await chmod(hostPath, 0o700);
    expect((await listVst3Classes({ bundlePath }, { hostPath })).classes.map((c) => c.name)).toEqual([
      "Instrument",
      "Effect",
    ]);
    await expect(listVst3Classes({ bundlePath, expectedHash: "b".repeat(64) }, { hostPath })).rejects.toMatchObject({
      code: "PluginManifestMismatch",
    });
  },
);
