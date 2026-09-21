// Exercise packed SDK/platform resolution outside the repository; no device or vendor plugin.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

assert.equal(process.platform, "darwin");
const root = realpathSync(mkdtempSync(join(tmpdir(), "oxitone-vst3-packed-")));
const output = resolve("target/ci");
mkdirSync(output, { recursive: true });
try {
  for (const name of ["protocol", "vst3", `vst3-host-darwin-${process.arch}`]) {
    const archive = join(output, `${name}.tgz`);
    execFileSync("pnpm", ["--filter", `@oxitone/${name}`, "pack", "--out", archive], { stdio: "pipe" });
    const destination = join(root, "node_modules", "@oxitone", name);
    mkdirSync(destination, { recursive: true });
    execFileSync("tar", ["-xzf", archive, "--strip-components=1", "-C", destination]);
  }
  const require = createRequire(new URL("../packages/protocol/package.json", import.meta.url));
  for (const [name, entry] of [
    ["zod", "zod/package.json"],
    ["@noble/hashes", "@noble/hashes/sha2.js"],
  ]) {
    const destination = join(root, "node_modules", name);
    mkdirSync(dirname(destination), { recursive: true });
    symlinkSync(dirname(require.resolve(entry)), destination, "dir");
  }
  writeFileSync(
    join(root, "check.mjs"),
    `import assert from 'node:assert/strict';
import { resolveVst3Host, inspectVst3Plugin, configureVst3Plugin } from '@oxitone/vst3';
const host = await resolveVst3Host({});
assert.ok(host.startsWith(${JSON.stringify(root)}), 'must resolve installed platform helper');
await assert.rejects(() => resolveVst3Host({ hostPath: '/missing-explicit-vst3-helper' }), { code: 'PluginHostUnavailable' });
await assert.rejects(() => inspectVst3Plugin({ bundlePath: ${JSON.stringify(join(root, "Missing.vst3"))}, classId: '1'.repeat(32), allowPlugins: 'any' }), { code: 'AssetUnavailable' });
await assert.rejects(() => configureVst3Plugin({ bundlePath: ${JSON.stringify(join(root, "Missing.vst3"))}, classId: '1'.repeat(32), allowPlugins: 'any' }), { code: 'AssetUnavailable' });
console.log('Packed VST3 SDK resolves and executes its platform helper; explicit invalid override fails without fallback');
`,
  );
  const env = { ...process.env };
  delete env.OXITONE_VST3_HOST_PATH;
  execFileSync(process.execPath, [join(root, "check.mjs")], { cwd: root, env, stdio: "inherit" });
} finally {
  rmSync(root, { recursive: true, force: true });
}
