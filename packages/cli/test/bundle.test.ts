import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { expect, it } from "vitest";
import type { PreviewFrame } from "@oxitone/protocol";
import { bundleProject } from "../src/bundle.js";
import { PreviewRunner } from "../src/preview/runner.js";
import { until } from "./preview-helpers.js";

it("bundles nested TS/JSON and literal dynamic imports into one executable file with original asset URLs", async () => {
  const root = fileURLToPath(new URL("../node_modules/.cache/", import.meta.url));
  await mkdir(root, { recursive: true });
  const dir = await mkdtemp(join(root, "bundle-")),
    child = join(dir, "parts"),
    out = join(dir, "output", "song.mjs");
  const frames: PreviewFrame[] = [];
  let runner: PreviewRunner | undefined;
  try {
    await mkdir(child);
    await writeFile(join(child, "name.txt"), "Bundled assets");
    await writeFile(join(child, "metadata.json"), '{"seed":42}');
    await writeFile(
      join(child, "data.ts"),
      `import { readFileSync } from 'node:fs';
      import metadata from './metadata.json';
      export const seed = metadata.seed;
      export const name = readFileSync(new URL('./name.txt', import.meta.url), 'utf8');
      export const where = import.meta.dirname;`,
    );
    const entry = join(dir, "song.ts");
    await writeFile(
      entry,
      `import { Project } from '@oxitone/core';
      export async function createProject() { const data = await import('./parts/data.js');
        return new Project({name:data.name, seed:data.seed}); }`,
    );
    await bundleProject(entry, out);
    const module = await import(pathToFileURL(out).href);
    expect(module.__oxitoneSourceDirectory).toBe(dir);
    expect((await module.createProject()).snapshot().name).toBe("Bundled assets");
    // The artifact must execute after its TS and JSON source dependencies are removed.
    await rm(entry);
    await rm(join(child, "data.ts"));
    await rm(join(child, "metadata.json"));
    runner = new PreviewRunner(out, (f) => frames.push(f), { watch: false, debounceMs: 0 });
    await runner.start();
    await until(() => frames.some((f) => f.type === "snapshot"));
    const snapshot = frames.find((f) => f.type === "snapshot")!;
    expect(snapshot.snapshot.name).toBe("Bundled assets");
    expect(snapshot.assetBaseDir).toBe(dir);
    expect(frames.some((f) => f.type === "diagnostic")).toBe(false);
  } finally {
    await runner?.close();
    await rm(dir, { recursive: true, force: true });
  }
});

it("retains the previous bundle after a broken build and rejects source overwrite", async () => {
  const root = fileURLToPath(new URL("../node_modules/.cache/", import.meta.url));
  await mkdir(root, { recursive: true });
  const dir = await mkdtemp(join(root, "bundle-failed-"));
  try {
    const entry = join(dir, "song.mjs"),
      out = join(dir, "compiled.mjs");
    await writeFile(entry, "export const project = 42;");
    await bundleProject(entry, out);
    const before = await readFile(out, "utf8");
    await expect(bundleProject(entry, entry)).rejects.toThrow("separate");
    await writeFile(entry, "export default = ;");
    await expect(bundleProject(entry, out)).rejects.toThrow();
    expect(await readFile(out, "utf8")).toBe(before);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
