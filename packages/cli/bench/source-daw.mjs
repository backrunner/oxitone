import { mkdir, mkdtemp, rm, symlink, writeFile } from "node:fs/promises";
import { cpus, platform, release, tmpdir } from "node:os";
import { join } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import { ProjectDocument } from "../dist/source/index.js";

const root = await mkdtemp(join(tmpdir(), "oxitone-daw-benchmark-"));
const iterations = 10,
  warmup = 1,
  measurements = [];
let document,
  step = 0;
async function measure(name, run, prepare = async () => {}) {
  const times = [];
  for (let i = 0; i < warmup + iterations; i++) {
    await prepare();
    const start = performance.now();
    await run();
    if (i >= warmup) times.push(performance.now() - start);
  }
  times.sort((a, b) => a - b);
  measurements.push({ name, p50Ms: times[4], p95Ms: times[9], p99Ms: times[9] });
}
async function edit() {
  const view = document.view,
    site = view.sites.find((site) => site.scope === "definition" && site.label === "phrase");
  await document.edit(view.revision, site.handle, [
    { select: { iteration: 2, note: { step: 1 } }, set: { pitch: 65 + (++step % 2) } },
  ]);
}
try {
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(fileURLToPath(new URL("../../core", import.meta.url)), join(root, "node_modules/@oxitone/core"), "dir");
  const entry = join(root, "song.ts");
  await writeFile(
    entry,
    "import { Project, chord, arp, wavetable, effect } from '@oxitone/core'; const phrase = arp(chord(60, 'major'), 'upDown', 0.25).repeat(25); const project = new Project({ sampleRate: 48000, blockSize: 128 }); const voice = wavetable(); const rack = [effect('delay'), effect('delay', { feedback: .6 })]; const channel = project.addChannel({ instrument: voice, effectChain: rack }); project.addTrack('First').use(channel).pattern(phrase).at({ bar: 1 }); project.addTrack('Second').use(channel).pattern(phrase).at({ bar: 2 }); export default project;",
  );
  document = await ProjectDocument.open({ entry });
  if (document.view.status !== "ready") throw new Error(JSON.stringify(document.view.diagnostic));
  await measure("whole-project-edit-validate-accept", edit);
  await measure("configuration-edit-validate-accept", async () => {
    const view = document.view,
      site = view.configurationSites.find((site) => site.scope === "reference" && site.label === "instrument");
    await document.editConfiguration(
      view.revision,
      site.handle,
      { kind: "parameters", values: { level: 0.3 + (++step % 2) / 10 } },
      site.usages[0].handle,
    );
  });
  await measure("rack-review-validate", async () => {
    const view = document.view,
      site = view.rackSites.find((site) => site.scope === "reference" && site.label === "effectChain");
    await document.planMaterializeRack(view.revision, site.handle, site.usages[0].owner);
    document.cancelMaterialize(view.revision, document.view.rackMaterialization.planId);
  });
  await measure("dirty-journal-save", () => document.save(document.view.revision), edit);
  await measure("effect-order-validate-accept", async () => {
    const view = document.view,
      channel = document.frame.snapshot.channels[0];
    const site = view.effectOwnerSites.find((site) => site.scope === "definition" && site.owner === channel.id);
    await document.editEffectOrder(
      view.revision,
      site.handle,
      channel.id,
      channel.effectChain.map((effect) => effect.instanceId).reverse(),
    );
  });
  await measure("source-and-plugin-projection", () => {
    JSON.stringify(document.view);
    JSON.stringify(document.frame);
  });
  await measure("mixer-configure-validate-accept", () =>
    document.configure(document.view.revision, {
      kind: "channel",
      index: 0,
      values: { level: 0.6 + (++step % 2) / 10 },
    }),
  );
  await measure("playlist-resize-validate-accept", () =>
    document.arrange(document.view.revision, {
      action: "resize",
      kind: "pattern",
      resource: 0,
      clip: 0,
      durationBeats: 80 + (++step % 2) * 4,
    }),
  );
  await measure("fresh-project-reopen", async () => {
    const reopened = await ProjectDocument.open({ entry });
    try {
      if (reopened.view.status !== "ready") throw new Error(JSON.stringify(reopened.view.diagnostic));
    } finally {
      reopened.close();
    }
  });
  console.log(
    JSON.stringify(
      {
        benchmark: "source-daw",
        cpu: cpus()[0]?.model,
        os: `${platform()} ${release()}`,
        node: process.version,
        notes: 100,
        placements: 2,
        plugins: document.view.plugins.length,
        iterations,
        warmup,
        sampleRate: 48000,
        blockSize: 128,
        device: null,
        callbackP95: null,
        callbackP99: null,
        xruns: null,
        measurements,
      },
      null,
      2,
    ),
  );
} finally {
  document?.close();
  await rm(root, { recursive: true, force: true });
}
