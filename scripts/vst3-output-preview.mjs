// Optional native routing capture with the same real VST3 registration and a simulated sink.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, symlink, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";

export async function captureOutputRoutes(project, root) {
  if (!process.env.OXITONE_VST3_VIEWER) return undefined;
  await mkdir(join(root, "node_modules", "@oxitone"), { recursive: true });
  await symlink(resolve("packages/core"), join(root, "node_modules", "@oxitone", "core"), "dir");
  const entry = join(root, "outputs.ts");
  await writeFile(
    entry,
    `import { Project } from '@oxitone/core';
const project = Project.fromSnapshot(${JSON.stringify(project.snapshot())});
for (const registration of ${JSON.stringify(project.registeredVst3Plugins)}) project.registerVst3(registration);
export default project;
`,
  );
  const path = resolve("target/vst3-output-routing.png");
  const result = spawnSync(
    process.execPath,
    [resolve("packages/cli/dist/index.js"), "preview", entry, "--viewer", resolve(process.env.OXITONE_VST3_VIEWER)],
    {
      encoding: "utf8",
      timeout: 45000,
      env: {
        ...process.env,
        OXITONE_PREVIEW_SIMULATED: "1",
        OXITONE_PREVIEW_CAPTURE: path,
        OXITONE_PREVIEW_CAPTURE_MIXER: "outputs",
        OXITONE_PREVIEW_APPEARANCE: "dark",
      },
    },
  );
  const output = `${result.stdout ?? ""}${result.stderr ?? ""}`;
  assert.equal(result.status, 0, output || String(result.error));
  assert.ok(output.includes("Preview instrument output routing smoke passed"), output);
  assert.ok(output.includes("Preview capture saved"), output);
  return path;
}
