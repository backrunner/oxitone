// Authoring/control-thread benchmark. Run after `pnpm --filter @oxitone/core build`.
import { cpus, release } from "node:os";
import { performance } from "node:perf_hooks";
import { canonicalEncode } from "../packages/protocol/dist/index.js";
import { AutomationSource, Pattern, Project } from "../packages/core/dist/index.js";

const project = new Project({ seed: 42 });
const bus = project.addMixerChannel({ level: 0.8 });
const fx = project.addMixerChannel({
  inserts: [{ pluginId: "oxitone.reverb", pluginVersion: "1.0.0", parameters: {} }],
});
bus.send(fx, { ratio: 0.25 });
for (let track = 0; track < 32; track++) {
  const channel = project.addChannel({ mixerChannelId: bus.id });
  project
    .addTrack()
    .use(channel)
    .add(
      new Pattern({
        id: `pat_bench_${track}`,
        lengthBeats: 16,
        notes: Array.from({ length: 64 }, (_, note) => ({
          pitch: 48 + (note % 24),
          start: note / 4,
          duration: 0.25,
          velocity: 0.5,
          chance: 0.8,
        })),
      }),
    )
    .at({ bar: 1 })
    .loop(4);
  channel.automate("level", new AutomationSource({ kind: "constant", value: 0.4 }));
}
const snapshot = project.snapshot();
const values = [];
for (let iteration = 0; iteration < 120; iteration++) {
  const start = performance.now();
  const restored = Project.fromSnapshot(snapshot).snapshot();
  const ms = performance.now() - start;
  if (iteration >= 20) values.push(ms);
  if (iteration === 0 && canonicalEncode(restored) !== canonicalEncode(snapshot))
    throw new Error("round-trip changed snapshot");
}
values.sort((a, b) => a - b);
process.stdout.write(
  JSON.stringify(
    {
      schema: "oxitone.project-restore-benchmark.v1",
      date: new Date().toISOString(),
      machine: { cpu: cpus()[0]?.model, osRelease: release(), arch: process.arch, node: process.version },
      config: {
        sampleRate: 48000,
        blockSize: 128,
        tracks: 32,
        channels: 32,
        patterns: 32,
        notesPerPattern: 64,
        patternClips: 32,
        lanes: 32,
        mixerBusesIncludingMaster: 3,
        sends: 1,
        warmupIterations: 20,
        measurementIterations: 100,
      },
      restoreAndSnapshot: { medianMs: (values[49] + values[50]) / 2, p95Ms: values[94], p99Ms: values[98] },
      samplesMs: values,
      callbackP95Ns: null,
      callbackP99Ns: null,
      xruns: null,
      notes: [
        "Measures schema/ownership validation, builder restoration and detached snapshot generation; fixture setup and canonical comparison are outside timing.",
        "Pure TypeScript control work; no native graph compilation, asset I/O, device, callback or rendering in the measured path.",
        "Initial baseline on a shared host; not realtime performance acceptance.",
      ],
    },
    null,
    2,
  ) + "\n",
);
