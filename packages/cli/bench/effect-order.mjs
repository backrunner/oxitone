import { channel } from "node:diagnostics_channel";
import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { cpus, platform, release, tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { ProjectDocument } from "../dist/source/index.js";

const root = await mkdtemp(join(tmpdir(), "oxitone-order-bench-"));
const iterations = 40,
  warmup = 5,
  samples = [];
const timing = channel("oxitone.source.timing");
let phases, document;
const collect = ({ phase, milliseconds }) => {
  if (phases) phases[phase] = (phases[phase] ?? 0) + milliseconds;
};
timing.subscribe(collect);
try {
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts");
  await writeFile(
    entry,
    "import { Project, chord, arp, wavetable, effect } from '@oxitone/core'; const phrase = arp(chord(60, 'major'), 'upDown', .25).repeat(25); const project = new Project({ sampleRate: 48000, blockSize: 128 }); const rack = [effect('delay'), effect('delay', { feedback: .6 })]; const channel = project.addChannel({ instrument: wavetable(), effectChain: rack }); project.addTrack('First').use(channel).pattern(phrase).at({ bar: 1 }); project.addTrack('Second').use(channel).pattern(phrase).at({ bar: 2 }); export default project;",
  );
  document = await ProjectDocument.open({ entry });
  if (document.view.status !== "ready") throw new Error(JSON.stringify(document.view.diagnostic));
  for (let i = 0; i < warmup + iterations; i++) {
    const view = document.view,
      owner = document.frame.snapshot.channels[0];
    const site = view.effectOwnerSites.find((site) => site.owner === owner.id && site.scope === "definition");
    phases = {};
    await document.editEffectOrder(
      view.revision,
      site.handle,
      owner.id,
      owner.effectChain.map((effect) => effect.instanceId).reverse(),
    );
    if (i >= warmup) samples.push(phases);
    phases = undefined;
  }
  const measurements = Object.keys(samples[0]).map((phase) => {
    const values = samples.map((sample) => sample[phase]).sort((a, b) => a - b);
    const percentile = (quantile) => values[Math.ceil(values.length * quantile) - 1];
    return { phase, p50Ms: percentile(0.5), p95Ms: percentile(0.95), p99Ms: percentile(0.99) };
  });
  console.log(
    JSON.stringify(
      {
        benchmark: "effect-order",
        cpu: cpus()[0]?.model,
        os: `${platform()} ${release()}`,
        node: process.version,
        iterations,
        warmup,
        notes: 100,
        placements: 2,
        effects: 2,
        sampleRate: 48000,
        blockSize: 128,
        nodeCompileCache: !!process.env.NODE_COMPILE_CACHE && process.env.NODE_DISABLE_COMPILE_CACHE !== "1",
        device: null,
        cpuUtilization: null,
        callbackP95: null,
        callbackP99: null,
        xruns: null,
        measurements,
        samples,
      },
      null,
      2,
    ),
  );
} finally {
  phases = undefined;
  timing.unsubscribe(collect);
  document?.close();
  await rm(root, { recursive: true, force: true });
}
