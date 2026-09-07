import { mkdir, mkdtemp, writeFile, rm } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { join } from "node:path";
import { expect, it } from "vitest";
import type { PreviewFrame } from "@oxitone/protocol";
import { PreviewRunner } from "../src/preview/runner.js";
import { until } from "./preview-helpers.js";

it("watches imported UI metadata independently of the musical snapshot and recovers from broken layout source", async () => {
  const cache = fileURLToPath(new URL("../node_modules/.cache/", import.meta.url));
  await mkdir(cache, { recursive: true });
  const dir = await mkdtemp(join(cache, "preview-ui-"));
  const entry = join(dir, "song.ts"), panel = join(dir, "panel.ts");
  const frames: PreviewFrame[] = [];
  const runner = new PreviewRunner(entry, (frame) => frames.push(frame), { debounceMs: 20 });
  const snapshots = () => frames.filter(f => f.type === "snapshot");
  const layout = (title: string, parameter = "filter.cutoff") => `export default {
    uiVersion:"1.0",pluginId:"oxitone.wavetable",pluginVersion:"1.0.0",title:"${title}",size:{width:500,height:320},
    pages:[{id:"main",title:"Main",groups:[{id:"tone",title:"Tone",columns:1,controls:[{kind:"knob",parameter:"${parameter}"}]}]}]
  };`;
  try {
    await writeFile(join(dir, "package.json"), '{"type":"module"}');
    await writeFile(entry, `import { Project } from '@oxitone/core'; import panel from './panel.js';
      export default () => new Project({seed:42}).registerPluginUi(panel);`);
    await writeFile(panel, layout("First"));
    await runner.start();
    await until(() => snapshots().length === 1);
    expect(frames.some(f => f.type === "status" && f.state === "watching")).toBe(false);
    await writeFile(panel, "export default {");
    await until(() => frames.some(f => f.type === "diagnostic"));
    expect(snapshots()).toHaveLength(1);
    await writeFile(panel, layout("Second"));
    await until(() => snapshots().length === 2);
    const [first, second] = snapshots();
    expect(first!.hash).not.toBe(second!.hash);
    expect({ ...first!.snapshot, revision: "0" }).toEqual({ ...second!.snapshot, revision: "0" });
    expect((second!.pluginUis as { title: string }[])[0]!.title).toBe("Second");
    await writeFile(panel, layout("Local fallback", "unknown"));
    await until(() => snapshots().length === 3);
    expect(snapshots()[2]!.snapshot.revision).toBe("3");
  } finally { await runner.close(); await rm(dir, { recursive: true, force: true }); }
}, 30_000);
